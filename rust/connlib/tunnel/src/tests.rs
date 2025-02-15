use crate::{ClientEvent, ClientState, GatewayEvent, GatewayState, Request};
use bimap::BiMap;
use chrono::{DateTime, Utc};
use connlib_shared::{
    messages::{
        client::{ResourceDescription, ResourceDescriptionCidr, SiteId},
        gateway, ClientId, DnsServer, GatewayId, Interface, RelayId, ResourceId,
    },
    proptest::cidr_resource,
    StaticSecret,
};
use firezone_relay::{AddressFamily, AllocationPort, ClientSocket, PeerSocket};
use ip_network::Ipv4Network;
use ip_network_table::IpNetworkTable;
use ip_packet::MutableIpPacket;
use pretty_assertions::assert_eq;
use proptest::{
    arbitrary::any,
    collection, sample,
    strategy::{Just, Strategy, Union},
    test_runner::Config,
};
use proptest_state_machine::{ReferenceStateMachine, StateMachineTest};
use rand::{rngs::StdRng, SeedableRng};
use secrecy::ExposeSecret;
use snownet::{RelaySocket, Transmit};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, VecDeque},
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    ops::ControlFlow,
    time::{Duration, Instant, SystemTime},
};
use tracing::{debug_span, error_span, subscriber::DefaultGuard, Span};
use tracing_subscriber::{util::SubscriberInitExt as _, EnvFilter};

proptest_state_machine::prop_state_machine! {
    #![proptest_config(Config {
        // Enable verbose mode to make the state machine test print the
        // transitions for each case.
        verbose: 1,
        cases: 1000,
        .. Config::default()
    })]

    #[test]
    fn run_tunnel_test(sequential 1..20 => TunnelTest);
}

/// The actual system-under-test.
///
/// [`proptest`] manipulates this using [`Transition`]s and we assert it against [`ReferenceState`].
struct TunnelTest {
    now: Instant,
    utc_now: DateTime<Utc>,

    client: SimNode<ClientId, ClientState>,
    gateway: SimNode<GatewayId, GatewayState>,
    relay: SimRelay<firezone_relay::Server<StdRng>>,
    portal: SimPortal,

    /// Bi-directional mapping between connlib's sentinel DNS IPs and the effective DNS servers.
    dns_by_sentinel: BiMap<IpAddr, SocketAddr>,

    client_sent_icmp_requests: HashMap<(u16, u16), ip_packet::IpPacket<'static>>,
    client_received_icmp_replies: HashMap<(u16, u16), ip_packet::IpPacket<'static>>,
    gateway_received_requests: HashMap<(u16, u16), ip_packet::IpPacket<'static>>,

    #[allow(dead_code)]
    logger: DefaultGuard,
}

/// The reference state machine of the tunnel.
///
/// This is the "expected" part of our test.
#[derive(Clone, Debug)]
struct ReferenceState {
    now: Instant,
    utc_now: DateTime<Utc>,
    client: SimNode<ClientId, PrivateKey>,
    gateway: SimNode<GatewayId, PrivateKey>,
    relay: SimRelay<u64>,

    /// The DNS resolvers configured on the client outside of connlib.
    system_dns_resolvers: Vec<IpAddr>,
    /// The upstream DNS resolvers configured in the portal.
    upstream_dns_resolvers: Vec<DnsServer>,

    /// Which resources the clients is aware of.
    client_cidr_resources: IpNetworkTable<ResourceDescriptionCidr>,
    /// The resources we have connected to.
    connected_resources: HashSet<ResourceId>,

    /// The expected ICMP handshakes (resource dst IP, seq, identifier)
    expected_icmp_handshakes: VecDeque<(IpAddr, u16, u16)>,
}

/// The possible transitions of the state machine.
#[derive(Clone, Debug)]
enum Transition {
    /// Add a new CIDR resource to the client.
    AddCidrResource(ResourceDescriptionCidr),
    SendICMPPacketToRandomIp {
        dst: IpAddr,
        seq: u16,
        identifier: u16,
    },
    /// Send a ICMP packet to an IPv4 resource.
    SendICMPPacketToIp4Resource {
        r_idx: sample::Index,
        seq: u16,
        identifier: u16,
    },
    /// Send a ICMP packet to an IPv6 resource.
    SendICMPPacketToIp6Resource {
        r_idx: sample::Index,
        seq: u16,
        identifier: u16,
    },
    /// The system's DNS servers changed.
    UpdateSystemDnsServers { servers: Vec<IpAddr> },
    /// The upstream DNS servers changed.
    UpdateUpstreamDnsServers { servers: Vec<DnsServer> },
    /// Advance time by this many milliseconds.
    Tick { millis: u64 },
}

impl StateMachineTest for TunnelTest {
    type SystemUnderTest = Self;
    type Reference = ReferenceState;

    // Initialize the system under test from our reference state.
    fn init_test(
        ref_state: &<Self::Reference as ReferenceStateMachine>::State,
    ) -> Self::SystemUnderTest {
        let logger = tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter(EnvFilter::from_default_env())
            .finish()
            .set_default();

        let mut client = ref_state.client.map_state(
            |key| ClientState::new(StaticSecret::from(key.0)),
            debug_span!("client"),
        );
        let mut gateway = ref_state.gateway.map_state(
            |key| GatewayState::new(StaticSecret::from(key.0)),
            debug_span!("gateway"),
        );
        let relay = SimRelay {
            state: firezone_relay::Server::new(
                ref_state.relay.ip_stack,
                rand::rngs::StdRng::seed_from_u64(ref_state.relay.state),
                49152,
                65535,
            ),
            ip_stack: ref_state.relay.ip_stack,
            id: ref_state.relay.id,
            span: error_span!("relay"),
            allocations: ref_state.relay.allocations.clone(),
            buffer: ref_state.relay.buffer.clone(),
        };

        client.state.update_relays(
            HashSet::default(),
            HashSet::from([relay.explode("client")]),
            ref_state.now,
        );
        let _ = client.state.update_interface_config(Interface {
            ipv4: client.tunnel_ip4,
            ipv6: client.tunnel_ip6,
            upstream_dns: ref_state.upstream_dns_resolvers.clone(),
        });
        let _ = client
            .state
            .update_system_resolvers(ref_state.system_dns_resolvers.clone());

        gateway.state.update_relays(
            HashSet::default(),
            HashSet::from([relay.explode("gateway")]),
            ref_state.now,
        );

        let portal = SimPortal {
            _client: client.id,
            gateway: gateway.id,
            _relay: relay.id,
        };

        let mut this = Self {
            now: ref_state.now,
            utc_now: ref_state.utc_now,
            client,
            gateway,
            portal,
            logger,
            relay,
            dns_by_sentinel: Default::default(),
            client_received_icmp_replies: Default::default(),
            client_sent_icmp_requests: Default::default(),
            gateway_received_requests: Default::default(),
        };

        let mut buffered_transmits = VecDeque::new();
        this.advance(ref_state, &mut buffered_transmits); // Perform initial setup before we apply the first transition.

        this
    }

    /// Apply a generated state transition to our system under test and assert against the reference state machine.
    ///
    /// This is equivalent to "arrange - act - assert" of a regular test:
    /// 1. We start out in a certain state (arrange)
    /// 2. We apply a [`Transition`] (act)
    /// 3. We assert against the reference state (assert)
    fn apply(
        mut state: Self::SystemUnderTest,
        ref_state: &<Self::Reference as ReferenceStateMachine>::State,
        transition: <Self::Reference as ReferenceStateMachine>::Transition,
    ) -> Self::SystemUnderTest {
        let mut buffered_transmits = VecDeque::new();

        // Act: Apply the transition
        match transition {
            Transition::AddCidrResource(r) => {
                state.client.span.in_scope(|| {
                    state
                        .client
                        .state
                        .add_resources(&[ResourceDescription::Cidr(r)]);
                });
            }
            Transition::SendICMPPacketToRandomIp {
                dst,
                seq,
                identifier,
            } => {
                let packet = ip_packet::make::icmp_request_packet(
                    state.client.tunnel_ip(dst),
                    dst,
                    seq,
                    identifier,
                );

                buffered_transmits.extend(state.send_ip_packet_client_to_gateway(packet));
            }
            Transition::SendICMPPacketToIp4Resource {
                r_idx,
                seq,
                identifier,
            } => {
                let dst = ref_state.sample_ipv4_cidr_resource_dst(&r_idx);
                let packet = ip_packet::make::icmp_request_packet(
                    state.client.tunnel_ip(dst),
                    dst,
                    seq,
                    identifier,
                );

                buffered_transmits.extend(state.send_ip_packet_client_to_gateway(packet));
            }
            Transition::SendICMPPacketToIp6Resource {
                r_idx,
                seq,
                identifier,
            } => {
                let dst = ref_state.sample_ipv6_cidr_resource_dst(&r_idx);
                let packet = ip_packet::make::icmp_request_packet(
                    state.client.tunnel_ip(dst),
                    dst,
                    seq,
                    identifier,
                );

                buffered_transmits.extend(state.send_ip_packet_client_to_gateway(packet))
            }
            Transition::Tick { millis } => {
                state.now += Duration::from_millis(millis);
            }
            Transition::UpdateSystemDnsServers { servers } => {
                let _ = state.client.state.update_system_resolvers(servers);
            }
            Transition::UpdateUpstreamDnsServers { servers } => {
                let _ = state.client.state.update_interface_config(Interface {
                    ipv4: state.client.tunnel_ip4,
                    ipv6: state.client.tunnel_ip6,
                    upstream_dns: servers,
                });
            }
        };
        state.advance(ref_state, &mut buffered_transmits);

        // Assert our properties: Check that our actual state is equivalent to our expectation (the reference state).
        for (resource_dst, seq, identifier) in ref_state.expected_icmp_handshakes.iter() {
            let client_sent_request = &state
                .client_sent_icmp_requests
                .get(&(*seq, *identifier))
                .unwrap();
            let client_received_reply = &state
                .client_received_icmp_replies
                .get(&(*seq, *identifier))
                .unwrap();
            let gateway_received_request = &state
                .gateway_received_requests
                .get(&(*seq, *identifier))
                .unwrap();

            assert_eq!(
                gateway_received_request.source(),
                ref_state
                    .client
                    .tunnel_ip(gateway_received_request.source()),
                "ICMP request on gateway to originate from client"
            );
            assert_eq!(
                gateway_received_request.destination(),
                *resource_dst,
                "ICMP request on gateway to target correct resource"
            );
            assert_eq!(
                client_sent_request.destination(),
                client_received_reply.source(),
                "ICMP request destination == ICMP reply source"
            );
            assert_eq!(
                client_sent_request.source(),
                client_received_reply.destination(),
                "ICMP request source == ICMP reply destination"
            );
        }
        assert_eq!(
            state.effective_dns_servers(),
            ref_state.expected_dns_servers(),
            "Effective DNS servers should match either system or upstream DNS"
        );

        assert!(buffered_transmits.is_empty()); // Sanity check to ensure we handled all packets.

        state
    }
}

/// Implementation of our reference state machine.
///
/// The logic in here represents what we expect the [`ClientState`] & [`GatewayState`] to do.
/// Care has to be taken that we don't implement things in a buggy way here.
/// After all, if your test has bugs, it won't catch any in the actual implementation.
impl ReferenceStateMachine for ReferenceState {
    type State = Self;
    type Transition = Transition;

    fn init_state() -> proptest::prelude::BoxedStrategy<Self::State> {
        (
            sim_node_prototype(client_id()),
            sim_node_prototype(gateway_id()),
            sim_relay_prototype(),
            system_dns_servers(),
            upstream_dns_servers(),
            Just(Instant::now()),
            Just(Utc::now()),
        )
            .prop_filter(
                "client and gateway priv key must be different",
                |(c, g, _, _, _, _, _)| c.state != g.state,
            )
            .prop_filter(
                "client, gateway and relay ip must be different",
                |(c, g, r, _, _, _, _)| {
                    let c4 = c.ip4_socket.map(|s| *s.ip());
                    let g4 = g.ip4_socket.map(|s| *s.ip());
                    let r4 = r.ip_stack.as_v4().copied();

                    let c6 = c.ip6_socket.map(|s| *s.ip());
                    let g6 = g.ip6_socket.map(|s| *s.ip());
                    let r6 = r.ip_stack.as_v6().copied();

                    let c4_eq_g4 = c4.is_some_and(|c| g4.is_some_and(|g| c == g));
                    let c6_eq_g6 = c6.is_some_and(|c| g6.is_some_and(|g| c == g));
                    let c4_eq_r4 = c4.is_some_and(|c| r4.is_some_and(|r| c == r));
                    let c6_eq_r6 = c6.is_some_and(|c| r6.is_some_and(|r| c == r));
                    let g4_eq_r4 = g4.is_some_and(|g| r4.is_some_and(|r| g == r));
                    let g6_eq_r6 = g6.is_some_and(|g| r6.is_some_and(|r| g == r));

                    !c4_eq_g4 && !c6_eq_g6 && !c4_eq_r4 && !c6_eq_r6 && !g4_eq_r4 && !g6_eq_r6
                },
            )
            .prop_map(
                |(
                    client,
                    gateway,
                    relay,
                    system_dns_resolvers,
                    upstream_dns_resolvers,
                    now,
                    utc_now,
                )| Self {
                    now,
                    utc_now,
                    client,
                    gateway,
                    relay,
                    system_dns_resolvers,
                    upstream_dns_resolvers,
                    client_cidr_resources: IpNetworkTable::new(),
                    connected_resources: Default::default(),
                    expected_icmp_handshakes: Default::default(),
                },
            )
            .boxed()
    }

    /// Defines the [`Strategy`] on how we can [transition](Transition) from the current [`ReferenceState`].
    ///
    /// This is invoked by proptest repeatedly to explore further state transitions.
    /// Here, we should only generate [`Transition`]s that make sense for the current state.
    fn transitions(state: &Self::State) -> proptest::prelude::BoxedStrategy<Self::Transition> {
        let add_cidr_resource = cidr_resource(8).prop_map(Transition::AddCidrResource);
        let tick = (0..=1000u64).prop_map(|millis| Transition::Tick { millis });
        let set_system_dns_servers =
            system_dns_servers().prop_map(|servers| Transition::UpdateSystemDnsServers { servers });
        let set_upstream_dns_servers = upstream_dns_servers()
            .prop_map(|servers| Transition::UpdateUpstreamDnsServers { servers });

        let (num_ip4_resources, num_ip6_resources) = state.client_cidr_resources.len();

        let mut strategies = vec![
            (1, add_cidr_resource.boxed()),
            (1, tick.boxed()),
            (1, set_system_dns_servers.boxed()),
            (1, set_upstream_dns_servers.boxed()),
            (1, icmp_to_random_ip().boxed()),
        ];

        if num_ip4_resources > 0 {
            strategies.push((3, icmp_to_ipv4_cidr_resource().boxed()));
        }

        if num_ip6_resources > 0 {
            strategies.push((3, icmp_to_ipv6_cidr_resource().boxed()));
        }

        Union::new_weighted(strategies).boxed()
    }

    /// Apply the transition to our reference state.
    ///
    /// Here is where we implement the "expected" logic.
    fn apply(mut state: Self::State, transition: &Self::Transition) -> Self::State {
        match transition {
            Transition::AddCidrResource(r) => {
                state.client_cidr_resources.insert(r.address, r.clone());
            }
            Transition::SendICMPPacketToRandomIp {
                dst,
                seq,
                identifier,
            } => {
                state.on_icmp_packet(*dst, *seq, *identifier);
            }
            Transition::SendICMPPacketToIp4Resource {
                r_idx,
                seq,
                identifier,
            } => {
                let dst = state.sample_ipv4_cidr_resource_dst(r_idx);
                state.on_icmp_packet(dst, *seq, *identifier);
            }
            Transition::SendICMPPacketToIp6Resource {
                r_idx,
                seq,
                identifier,
            } => {
                let dst = state.sample_ipv6_cidr_resource_dst(r_idx);
                state.on_icmp_packet(dst, *seq, *identifier);
            }
            Transition::Tick { millis } => state.now += Duration::from_millis(*millis),
            Transition::UpdateSystemDnsServers { servers } => {
                state.system_dns_resolvers.clone_from(servers);
            }
            Transition::UpdateUpstreamDnsServers { servers } => {
                state.upstream_dns_resolvers.clone_from(servers);
            }
        };

        state
    }

    /// Any additional checks on whether a particular [`Transition`] can be applied to a certain state.
    fn preconditions(state: &Self::State, transition: &Self::Transition) -> bool {
        match transition {
            Transition::AddCidrResource(r) => {
                // TODO: PRODUCTION CODE DOES NOT HANDLE THIS!

                if r.address.is_ipv6() && state.gateway.ip6_socket.is_none() {
                    return false;
                }

                if r.address.is_ipv4() && state.gateway.ip4_socket.is_none() {
                    return false;
                }

                true
            }
            Transition::Tick { .. } => true,
            Transition::SendICMPPacketToRandomIp {
                dst,
                seq,
                identifier,
            } => state.is_valid_icmp_packet(dst, seq, identifier),
            Transition::SendICMPPacketToIp4Resource {
                r_idx,
                seq,
                identifier,
            } => {
                if state.client_cidr_resources.len().0 == 0 {
                    return false;
                }

                let dst = state.sample_ipv4_cidr_resource_dst(r_idx);

                state.is_valid_icmp_packet(&dst.into(), seq, identifier)
            }
            Transition::SendICMPPacketToIp6Resource {
                r_idx,
                seq,
                identifier,
            } => {
                if state.client_cidr_resources.len().1 == 0 {
                    return false;
                }

                let dst = state.sample_ipv6_cidr_resource_dst(r_idx);

                state.is_valid_icmp_packet(&dst.into(), seq, identifier)
            }
            Transition::UpdateSystemDnsServers { .. } => true,
            Transition::UpdateUpstreamDnsServers { .. } => true,
        }
    }
}

impl TunnelTest {
    /// Exhaustively advances all state machines (client, gateway & relay).
    ///
    /// For our tests to work properly, each [`Transition`] needs to advance the state as much as possible.
    /// For example, upon the first packet to a resource, we need to trigger the connection intent and fully establish a connection.
    /// Dispatching a [`Transmit`] (read: packet) to a component can trigger more packets, i.e. receiving a STUN request may trigger a STUN response.
    ///
    /// Consequently, this function needs to loop until no component can make progress at which point we consider the [`Transition`] complete.
    fn advance(
        &mut self,
        ref_state: &ReferenceState,
        buffered_transmits: &mut VecDeque<(Transmit<'static>, Option<SocketAddr>)>,
    ) {
        loop {
            if let Some((transmit, sending_socket)) = buffered_transmits.pop_front() {
                self.dispatch_transmit(transmit, sending_socket, buffered_transmits);
                continue;
            }

            if let Some(transmit) = self.client.state.poll_transmit() {
                let sending_socket = self.client.sending_socket_for(transmit.dst);

                buffered_transmits.push_back((transmit, sending_socket));
                continue;
            }
            if let Some(event) = self.client.state.poll_event() {
                self.on_client_event(self.client.id, event, &ref_state.client_cidr_resources);
                continue;
            }
            if let Some(transmit) = self.gateway.state.poll_transmit() {
                let sending_socket = self.gateway.sending_socket_for(transmit.dst);

                buffered_transmits.push_back((transmit, sending_socket));
                continue;
            }
            if let Some(event) = self.gateway.state.poll_event() {
                self.on_gateway_event(self.gateway.id, event);
                continue;
            }
            if let Some(message) = self.relay.state.next_command() {
                match message {
                    firezone_relay::Command::SendMessage { payload, recipient } => {
                        let dst = recipient.into_socket();
                        let src = self
                            .relay
                            .sending_socket_for(dst, 3478)
                            .expect("relay to never emit packets without a matching socket");

                        if let ControlFlow::Break(_) = self.try_handle_client(dst, src, &payload) {
                            continue;
                        }

                        if let ControlFlow::Break(_) =
                            self.try_handle_gateway(dst, src, &payload, buffered_transmits)
                        {
                            continue;
                        }

                        panic!("Unhandled packet: {src} -> {dst}")
                    }

                    firezone_relay::Command::CreateAllocation { port, family } => {
                        self.relay.allocations.insert((family, port));
                    }
                    firezone_relay::Command::FreeAllocation { port, family } => {
                        self.relay.allocations.remove(&(family, port));
                    }
                }
                continue;
            }

            if self.handle_timeout(self.now, self.utc_now) {
                continue;
            }

            break;
        }
    }

    /// Returns the _effective_ DNS servers that connlib is using.
    fn effective_dns_servers(&self) -> HashSet<SocketAddr> {
        self.dns_by_sentinel.right_values().copied().collect()
    }

    /// Forwards time to the given instant iff the corresponding component would like that (i.e. returns a timestamp <= from `poll_timeout`).
    ///
    /// Tying the forwarding of time to the result of `poll_timeout` gives us better coverage because in production, we suspend until the value of `poll_timeout`.
    fn handle_timeout(&mut self, now: Instant, utc_now: DateTime<Utc>) -> bool {
        let mut any_advanced = false;

        if self.client.state.poll_timeout().is_some_and(|t| t <= now) {
            any_advanced = true;

            self.client
                .span
                .in_scope(|| self.client.state.handle_timeout(now));
        };

        if self.gateway.state.poll_timeout().is_some_and(|t| t <= now) {
            any_advanced = true;

            self.gateway
                .span
                .in_scope(|| self.gateway.state.handle_timeout(now, utc_now))
        };

        if self.relay.state.poll_timeout().is_some_and(|t| t <= now) {
            any_advanced = true;

            self.relay
                .span
                .in_scope(|| self.relay.state.handle_timeout(now))
        };

        any_advanced
    }

    fn send_ip_packet_client_to_gateway(
        &mut self,
        packet: MutableIpPacket<'_>,
    ) -> Option<(Transmit<'static>, Option<SocketAddr>)> {
        {
            let packet = packet.to_owned().into_immutable();

            if let Some(icmp) = packet.as_icmp() {
                let echo_request = icmp.as_echo_request().expect("to be echo request");

                self.client_sent_icmp_requests
                    .insert((echo_request.sequence(), echo_request.identifier()), packet);
            }
        }

        let transmit = self
            .client
            .span
            .in_scope(|| self.client.state.encapsulate(packet, self.now))?;
        let transmit = transmit.into_owned();
        let sending_socket = self.client.sending_socket_for(transmit.dst);

        Some((transmit, sending_socket))
    }

    /// Dispatches a [`Transmit`] to the correct component.
    ///
    /// This function is basically the "network layer" of our tests.
    /// It takes a [`Transmit`] and checks, which component accepts it, i.e. has configured the correct IP address.
    /// Our tests don't have a concept of a network topology.
    /// This means, components can have IP addresses in completely different subnets, yet this function will still "route" them correctly.
    fn dispatch_transmit(
        &mut self,
        transmit: Transmit,
        sending_socket: Option<SocketAddr>,
        buffered_transmits: &mut VecDeque<(Transmit<'static>, Option<SocketAddr>)>,
    ) {
        let dst = transmit.dst;
        let payload = &transmit.payload;

        let Some(src) = sending_socket else {
            tracing::warn!("Dropping packet to {dst}: no socket");
            return;
        };

        if self
            .try_handle_relay(dst, src, payload, buffered_transmits)
            .is_break()
        {
            return;
        }

        let src = transmit
            .src
            .expect("all packets without src should have been handled via relays");

        if self.try_handle_client(dst, src, payload).is_break() {
            return;
        }

        if self
            .try_handle_gateway(dst, src, payload, buffered_transmits)
            .is_break()
        {
            return;
        }

        panic!("Unhandled packet: {src} -> {dst}")
    }

    fn try_handle_relay(
        &mut self,
        dst: SocketAddr,
        src: SocketAddr,
        payload: &[u8],
        buffered_transmits: &mut VecDeque<(Transmit<'static>, Option<SocketAddr>)>,
    ) -> ControlFlow<()> {
        if !self.relay.wants(dst) {
            return ControlFlow::Continue(());
        }

        self.relay
            .handle_packet(payload, src, dst, self.now, buffered_transmits);

        ControlFlow::Break(())
    }

    fn try_handle_client(
        &mut self,
        dst: SocketAddr,
        src: SocketAddr,
        payload: &[u8],
    ) -> ControlFlow<()> {
        let mut buffer = [0u8; 200]; // In these tests, we only send ICMP packets which are very small.

        if !self.client.wants(dst) {
            return ControlFlow::Continue(());
        }

        if let Some(packet) = self.client.span.in_scope(|| {
            self.client
                .state
                .decapsulate(dst, src, payload, self.now, &mut buffer)
        }) {
            let icmp = packet.as_icmp().expect("to be ICMP packet");
            let echo_reply = icmp.as_echo_reply().expect("to be echo reply");

            self.client_received_icmp_replies.insert(
                (echo_reply.sequence(), echo_reply.identifier()),
                packet.to_owned(),
            );
        };

        ControlFlow::Break(())
    }

    fn try_handle_gateway(
        &mut self,
        dst: SocketAddr,
        src: SocketAddr,
        payload: &[u8],
        buffered_transmits: &mut VecDeque<(Transmit<'static>, Option<SocketAddr>)>,
    ) -> ControlFlow<()> {
        let mut buffer = [0u8; 200]; // In these tests, we only send ICMP packets which are very small.

        if !self.gateway.wants(dst) {
            return ControlFlow::Continue(());
        }

        if let Some(packet) = self.gateway.span.in_scope(|| {
            self.gateway
                .state
                .decapsulate(dst, src, payload, self.now, &mut buffer)
        }) {
            let packet = packet.to_owned();

            let icmp = packet.as_icmp().expect("to be ICMP packet");
            let echo_request = icmp.as_echo_request().expect("to be echo request");

            self.gateway_received_requests.insert(
                (echo_request.sequence(), echo_request.identifier()),
                packet.clone(),
            );

            if let Some(transmit) = self.gateway.span.in_scope(|| {
                self.gateway
                    .state
                    .encapsulate(ip_packet::make::icmp_response_packet(packet), self.now)
            }) {
                let transmit = transmit.into_owned();
                let dst = transmit.dst;

                buffered_transmits.push_back((transmit, self.gateway.sending_socket_for(dst)));
            };
        };

        ControlFlow::Break(())
    }

    fn on_client_event(
        &mut self,
        src: ClientId,
        event: ClientEvent,
        client_cidr_resources: &IpNetworkTable<ResourceDescriptionCidr>,
    ) {
        match event {
            ClientEvent::NewIceCandidate { candidate, .. } => self.gateway.span.in_scope(|| {
                self.gateway
                    .state
                    .add_ice_candidate(src, candidate, self.now)
            }),
            ClientEvent::InvalidatedIceCandidate { candidate, .. } => self
                .gateway
                .span
                .in_scope(|| self.gateway.state.remove_ice_candidate(src, candidate)),
            ClientEvent::ConnectionIntent {
                resource,
                connected_gateway_ids,
            } => {
                let (gateway, site) = self.portal.handle_connection_intent(
                    resource,
                    connected_gateway_ids,
                    client_cidr_resources,
                );

                // TODO: All of the below should be somehow encapsulated in `SimPortal`.

                let request = self
                    .client
                    .span
                    .in_scope(|| {
                        self.client.state.create_or_reuse_connection(
                            resource,
                            gateway,
                            site,
                            HashSet::default(),
                            HashSet::default(),
                        )
                    })
                    .unwrap();

                let resource_id = request.resource_id();
                // TODO: For DNS resources, we need to come up with an IP that our resource resolves to on the other side.
                let resource =
                    map_client_resource_to_gateway_resource(client_cidr_resources, resource_id);

                match request {
                    Request::NewConnection(new_connection) => {
                        let connection_accepted = self
                            .gateway
                            .span
                            .in_scope(|| {
                                self.gateway.state.accept(
                                    self.client.id,
                                    snownet::Offer {
                                        session_key: new_connection
                                            .client_preshared_key
                                            .expose_secret()
                                            .0
                                            .into(),
                                        credentials: snownet::Credentials {
                                            username: new_connection
                                                .client_payload
                                                .ice_parameters
                                                .username,
                                            password: new_connection
                                                .client_payload
                                                .ice_parameters
                                                .password,
                                        },
                                    },
                                    self.client.state.public_key(),
                                    vec![
                                        self.client.tunnel_ip4.into(),
                                        self.client.tunnel_ip6.into(),
                                    ],
                                    HashSet::default(),
                                    HashSet::default(),
                                    new_connection.client_payload.domain,
                                    None, // TODO: How to generate expiry?
                                    resource,
                                    self.now,
                                )
                            })
                            .unwrap();

                        self.client
                            .span
                            .in_scope(|| {
                                self.client.state.accept_answer(
                                    snownet::Answer {
                                        credentials: snownet::Credentials {
                                            username: connection_accepted.ice_parameters.username,
                                            password: connection_accepted.ice_parameters.password,
                                        },
                                    },
                                    resource_id,
                                    self.gateway.state.public_key(),
                                    connection_accepted.domain_response,
                                    self.now,
                                )
                            })
                            .unwrap();
                    }
                    Request::ReuseConnection(reuse_connection) => {
                        if let Some(domain_response) = self.gateway.span.in_scope(|| {
                            self.gateway.state.allow_access(
                                resource,
                                self.client.id,
                                None,
                                reuse_connection.payload,
                            )
                        }) {
                            self.client
                                .span
                                .in_scope(|| {
                                    self.client
                                        .state
                                        .received_domain_parameters(resource_id, domain_response)
                                })
                                .unwrap();
                        };
                    }
                };
            }
            ClientEvent::RefreshResources { .. } => {
                tracing::warn!("Unimplemented");
            }
            ClientEvent::ResourcesChanged { .. } => {
                tracing::warn!("Unimplemented");
            }
            ClientEvent::DnsServersChanged { dns_by_sentinel } => {
                self.dns_by_sentinel = dns_by_sentinel;
            }
        }
    }

    fn on_gateway_event(&mut self, src: GatewayId, event: GatewayEvent) {
        match event {
            GatewayEvent::NewIceCandidate { candidate, .. } => self.client.span.in_scope(|| {
                self.client
                    .state
                    .add_ice_candidate(src, candidate, self.now)
            }),
            GatewayEvent::InvalidIceCandidate { candidate, .. } => self
                .client
                .span
                .in_scope(|| self.client.state.remove_ice_candidate(src, candidate)),
        }
    }
}

/// Several helper functions to make the reference state more readable.
impl ReferenceState {
    #[tracing::instrument(level = "debug", skip_all, fields(dst, resource))]
    fn on_icmp_packet(&mut self, dst: impl Into<IpAddr>, seq: u16, identifier: u16) {
        let dst = dst.into();

        tracing::Span::current().record("dst", tracing::field::display(dst));
        // Second, if we are not yet connected, check if we have a resource for this IP.
        let Some((_, resource)) = self.client_cidr_resources.longest_match(dst) else {
            tracing::debug!("No resource corresponds to IP");
            return;
        };

        if self.connected_resources.contains(&resource.id) {
            tracing::debug!("Connected to resource, expecting packet to be routed");
            self.expected_icmp_handshakes
                .push_back((dst, seq, identifier));
            return;
        }

        // If we have a resource, the first packet will initiate a connection to the gateway.
        tracing::debug!("Not connected to resource, expecting to trigger connection intent");
        self.connected_resources.insert(resource.id);
    }

    /// Samples an [`Ipv4Addr`] from _any_ of our IPv4 CIDR resources.
    fn sample_ipv4_cidr_resource_dst(&self, idx: &sample::Index) -> Ipv4Addr {
        let num_ip4_resources = self.client_cidr_resources.len().0;
        debug_assert!(num_ip4_resources > 0, "cannot sample without any resources");
        let r_idx = idx.index(num_ip4_resources);
        let (network, _) = self
            .client_cidr_resources
            .iter_ipv4()
            .nth(r_idx)
            .expect("index to be in range");

        let num_hosts = network.hosts().len();

        if num_hosts == 0 {
            debug_assert!(network.netmask() == 31 || network.netmask() == 32); // /31 and /32 don't have any hosts

            return network.network_address();
        }

        let addr_idx = idx.index(num_hosts);

        network.hosts().nth(addr_idx).expect("index to be in range")
    }

    /// Samples an [`Ipv6Addr`] from _any_ of our IPv6 CIDR resources.
    fn sample_ipv6_cidr_resource_dst(&self, idx: &sample::Index) -> Ipv6Addr {
        let num_ip6_resources = self.client_cidr_resources.len().1;
        debug_assert!(num_ip6_resources > 0, "cannot sample without any resources");
        let r_idx = idx.index(num_ip6_resources);
        let (network, _) = self
            .client_cidr_resources
            .iter_ipv6()
            .nth(r_idx)
            .expect("index to be in range");

        let num_hosts = network.subnets_with_prefix(128).len();

        let network = if num_hosts == 0 {
            debug_assert!(network.netmask() == 127 || network.netmask() == 128); // /127 and /128 don't have any hosts

            network
        } else {
            let addr_idx = idx.index(num_hosts);

            network
                .subnets_with_prefix(128)
                .nth(addr_idx)
                .expect("index to be in range")
        };

        network.network_address()
    }

    /// An ICMP packet is valid if it doesn't target the client directly and we didn't yet send an ICMP packet with the same seq and identifier.
    fn is_valid_icmp_packet(&self, dst: &IpAddr, seq: &u16, identifier: &u16) -> bool {
        let not_self_ping = match dst {
            IpAddr::V4(dst) => dst != &self.client.tunnel_ip4,
            IpAddr::V6(dst) => dst != &self.client.tunnel_ip6,
        };
        let not_existing_icmp =
            self.expected_icmp_handshakes
                .iter()
                .all(|(_, existing_seq, existing_identifer)| {
                    existing_seq != seq && existing_identifer != identifier
                });

        not_self_ping && not_existing_icmp
    }

    /// Returns the DNS servers that we expect connlib to use.
    ///
    /// If there are upstream DNS servers configured in the portal, it should use those.
    /// Otherwise it should use whatever was configured on the system prior to connlib starting.
    fn expected_dns_servers(&self) -> HashSet<SocketAddr> {
        if !self.upstream_dns_resolvers.is_empty() {
            return self
                .upstream_dns_resolvers
                .iter()
                .map(|s| s.address())
                .collect();
        }

        self.system_dns_resolvers
            .iter()
            .map(|ip| SocketAddr::new(*ip, 53))
            .collect()
    }
}

#[derive(Clone)]
struct SimNode<ID, S> {
    id: ID,
    state: S,

    ip4_socket: Option<SocketAddrV4>,
    ip6_socket: Option<SocketAddrV6>,

    tunnel_ip4: Ipv4Addr,
    tunnel_ip6: Ipv6Addr,

    span: Span,
}

#[derive(Clone)]
struct SimRelay<S> {
    id: RelayId,
    state: S,

    ip_stack: firezone_relay::IpStack,
    allocations: HashSet<(AddressFamily, AllocationPort)>,
    buffer: Vec<u8>,

    span: Span,
}

/// Stub implementation of the portal.
///
/// Currently, we only simulate a connection between a single client and a single gateway on a single site.
#[derive(Debug, Clone)]
struct SimPortal {
    _client: ClientId,
    gateway: GatewayId,
    _relay: RelayId,
}

impl<ID, S> SimNode<ID, S>
where
    ID: Copy,
    S: Copy,
{
    fn map_state<T>(&self, f: impl FnOnce(S) -> T, span: Span) -> SimNode<ID, T> {
        SimNode {
            id: self.id,
            state: f(self.state),
            ip4_socket: self.ip4_socket,
            ip6_socket: self.ip6_socket,
            tunnel_ip4: self.tunnel_ip4,
            tunnel_ip6: self.tunnel_ip6,
            span,
        }
    }
}

impl<ID, S> SimNode<ID, S> {
    fn wants(&self, dst: SocketAddr) -> bool {
        self.ip4_socket.is_some_and(|s| SocketAddr::V4(s) == dst)
            || self.ip6_socket.is_some_and(|s| SocketAddr::V6(s) == dst)
    }

    fn sending_socket_for(&self, dst: SocketAddr) -> Option<SocketAddr> {
        Some(match dst {
            SocketAddr::V4(_) => self.ip4_socket?.into(),
            SocketAddr::V6(_) => self.ip6_socket?.into(),
        })
    }

    fn tunnel_ip(&self, dst: impl Into<IpAddr>) -> IpAddr {
        match dst.into() {
            IpAddr::V4(_) => IpAddr::from(self.tunnel_ip4),
            IpAddr::V6(_) => IpAddr::from(self.tunnel_ip6),
        }
    }
}

impl SimRelay<firezone_relay::Server<StdRng>> {
    fn wants(&self, dst: SocketAddr) -> bool {
        let is_direct = self.matching_listen_socket(dst).is_some_and(|s| s == dst);
        let is_allocation_port = self.allocations.contains(&match dst {
            SocketAddr::V4(_) => (AddressFamily::V4, AllocationPort::new(dst.port())),
            SocketAddr::V6(_) => (AddressFamily::V6, AllocationPort::new(dst.port())),
        });
        let is_allocation_ip = self
            .matching_listen_socket(dst)
            .is_some_and(|s| s.ip() == dst.ip());

        is_direct || (is_allocation_port && is_allocation_ip)
    }

    fn sending_socket_for(&self, dst: SocketAddr, port: u16) -> Option<SocketAddr> {
        Some(match dst {
            SocketAddr::V4(_) => SocketAddr::V4(SocketAddrV4::new(*self.ip_stack.as_v4()?, port)),
            SocketAddr::V6(_) => {
                SocketAddr::V6(SocketAddrV6::new(*self.ip_stack.as_v6()?, port, 0, 0))
            }
        })
    }

    fn explode(&self, username: &str) -> (RelayId, RelaySocket, String, String, String) {
        let relay_socket = match self.ip_stack {
            firezone_relay::IpStack::Ip4(ip4) => RelaySocket::V4(SocketAddrV4::new(ip4, 3478)),
            firezone_relay::IpStack::Ip6(ip6) => {
                RelaySocket::V6(SocketAddrV6::new(ip6, 3478, 0, 0))
            }
            firezone_relay::IpStack::Dual { ip4, ip6 } => RelaySocket::Dual {
                v4: SocketAddrV4::new(ip4, 3478),
                v6: SocketAddrV6::new(ip6, 3478, 0, 0),
            },
        };

        let (username, password) = self.make_credentials(username);

        (
            self.id,
            relay_socket,
            username,
            password,
            "firezone".to_owned(),
        )
    }

    fn matching_listen_socket(&self, other: SocketAddr) -> Option<SocketAddr> {
        match other {
            SocketAddr::V4(_) => Some(SocketAddr::new((*self.ip_stack.as_v4()?).into(), 3478)),
            SocketAddr::V6(_) => Some(SocketAddr::new((*self.ip_stack.as_v6()?).into(), 3478)),
        }
    }

    fn ip4(&self) -> Option<IpAddr> {
        self.ip_stack.as_v4().copied().map(|i| i.into())
    }

    fn ip6(&self) -> Option<IpAddr> {
        self.ip_stack.as_v6().copied().map(|i| i.into())
    }

    fn handle_packet(
        &mut self,
        payload: &[u8],
        sender: SocketAddr,
        dst: SocketAddr,
        now: Instant,
        buffered_transmits: &mut VecDeque<(Transmit<'static>, Option<SocketAddr>)>,
    ) {
        if self.matching_listen_socket(dst).is_some_and(|s| s == dst) {
            self.handle_client_input(payload, ClientSocket::new(sender), now, buffered_transmits);
            return;
        }

        self.handle_peer_traffic(
            payload,
            PeerSocket::new(sender),
            AllocationPort::new(dst.port()),
            buffered_transmits,
        )
    }

    fn handle_client_input(
        &mut self,
        payload: &[u8],
        client: ClientSocket,
        now: Instant,
        buffered_transmits: &mut VecDeque<(Transmit<'static>, Option<SocketAddr>)>,
    ) {
        if let Some((port, peer)) = self
            .span
            .in_scope(|| self.state.handle_client_input(payload, client, now))
        {
            let payload = &payload[4..];

            // The `dst` of the relayed packet is what TURN calls a "peer".
            let dst = peer.into_socket();

            // The `src_ip` is the relay's IP
            let src_ip = match dst {
                SocketAddr::V4(_) => {
                    assert!(
                        self.allocations.contains(&(AddressFamily::V4, port)),
                        "IPv4 allocation to be present if we want to send to an IPv4 socket"
                    );

                    self.ip4().expect("listen on IPv4 if we have an allocation")
                }
                SocketAddr::V6(_) => {
                    assert!(
                        self.allocations.contains(&(AddressFamily::V6, port)),
                        "IPv6 allocation to be present if we want to send to an IPv6 socket"
                    );

                    self.ip6().expect("listen on IPv6 if we have an allocation")
                }
            };

            // The `src` of the relayed packet is the relay itself _from_ the allocated port.
            let src = SocketAddr::new(src_ip, port.value());

            // Check if we need to relay to ourselves (from one allocation to another)
            if self.wants(dst) {
                // When relaying to ourselves, we become our own peer.
                let peer_socket = PeerSocket::new(src);
                // The allocation that the data is arriving on is the `dst`'s port.
                let allocation_port = AllocationPort::new(dst.port());

                self.handle_peer_traffic(payload, peer_socket, allocation_port, buffered_transmits);
                return;
            }

            buffered_transmits.push_back((
                Transmit {
                    src: Some(src),
                    dst,
                    payload: Cow::Owned(payload.to_vec()),
                },
                Some(src),
            ));
        }
    }

    fn handle_peer_traffic(
        &mut self,
        payload: &[u8],
        peer: PeerSocket,
        port: AllocationPort,
        buffered_transmits: &mut VecDeque<(Transmit<'static>, Option<SocketAddr>)>,
    ) {
        if let Some((client, channel)) = self
            .span
            .in_scope(|| self.state.handle_peer_traffic(payload, peer, port))
        {
            let full_length = firezone_relay::ChannelData::encode_header_to_slice(
                channel,
                payload.len() as u16,
                &mut self.buffer[..4],
            );
            self.buffer[4..full_length].copy_from_slice(payload);

            let receiving_socket = client.into_socket();
            let sending_socket = self.matching_listen_socket(receiving_socket).unwrap();

            buffered_transmits.push_back((
                Transmit {
                    src: Some(sending_socket),
                    dst: receiving_socket,
                    payload: Cow::Owned(self.buffer[..full_length].to_vec()),
                },
                Some(sending_socket),
            ));
        }
    }

    fn make_credentials(&self, username: &str) -> (String, String) {
        let expiry = SystemTime::now() + Duration::from_secs(60);

        let secs = expiry
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("expiry must be later than UNIX_EPOCH")
            .as_secs();

        let password =
            firezone_relay::auth::generate_password(self.state.auth_secret(), expiry, username);

        (format!("{secs}:{username}"), password)
    }
}

impl SimPortal {
    /// Picks, which gateway and site we should connect to for the given resource.
    fn handle_connection_intent(
        &self,
        resource: ResourceId,
        _connected_gateway_ids: HashSet<GatewayId>,
        client_cidr_resources: &IpNetworkTable<ResourceDescriptionCidr>,
    ) -> (GatewayId, SiteId) {
        // TODO: Should we somehow vary how many gateways we connect to?
        // TODO: Should we somehow pick, which site to use?

        let site = client_cidr_resources
            .iter()
            .find_map(|(_, r)| (r.id == resource).then_some(r.sites.first()?.id))
            .expect("resource to have at least 1 site");

        (self.gateway, site)
    }
}

impl<ID: fmt::Debug, S: fmt::Debug> fmt::Debug for SimNode<ID, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimNode")
            .field("id", &self.id)
            .field("state", &self.state)
            .field("ip4_socket", &self.ip4_socket)
            .field("ip6_socket", &self.ip6_socket)
            .field("tunnel_ip4", &self.tunnel_ip4)
            .field("tunnel_ip6", &self.tunnel_ip6)
            .finish()
    }
}

impl<S: fmt::Debug> fmt::Debug for SimRelay<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimRelay")
            .field("id", &self.id)
            .field("ip_stack", &self.ip_stack)
            .field("allocations", &self.allocations)
            .finish()
    }
}

fn map_client_resource_to_gateway_resource(
    client_cidr_resource: &IpNetworkTable<ResourceDescriptionCidr>,
    resource_id: ResourceId,
) -> gateway::ResourceDescription<gateway::ResolvedResourceDescriptionDns> {
    let client_resource = client_cidr_resource
        .iter()
        .find_map(|(_, r)| (r.id == resource_id).then_some(r.clone()))
        .expect("to know about ID");

    gateway::ResourceDescription::<gateway::ResolvedResourceDescriptionDns>::Cidr(
        gateway::ResourceDescriptionCidr {
            id: client_resource.id,
            address: client_resource.address,
            name: client_resource.name,
            filters: Vec::new(),
        },
    )
}

#[derive(Clone, Copy, PartialEq)]
struct PrivateKey([u8; 32]);

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PrivateKey")
            .field(&hex::encode(self.0))
            .finish()
    }
}

/// Generates a [`Transition`] that sends an ICMP packet to a random IP.
///
/// By chance, it could be that we pick a resource IP here.
/// That is okay as our reference state machine checks separately whether we are pinging a resource here.
fn icmp_to_random_ip() -> impl Strategy<Value = Transition> {
    (any::<IpAddr>(), any::<u16>(), any::<u16>()).prop_map(|(dst, seq, identifier)| {
        Transition::SendICMPPacketToRandomIp {
            dst,
            seq,
            identifier,
        }
    })
}

fn icmp_to_ipv4_cidr_resource() -> impl Strategy<Value = Transition> {
    (any::<sample::Index>(), any::<u16>(), any::<u16>()).prop_map(
        move |(r_idx, seq, identifier)| Transition::SendICMPPacketToIp4Resource {
            r_idx,
            seq,
            identifier,
        },
    )
}

fn icmp_to_ipv6_cidr_resource() -> impl Strategy<Value = Transition> {
    (any::<sample::Index>(), any::<u16>(), any::<u16>()).prop_map(
        move |(r_idx, seq, identifier)| Transition::SendICMPPacketToIp6Resource {
            r_idx,
            seq,
            identifier,
        },
    )
}

fn client_id() -> impl Strategy<Value = ClientId> {
    (any::<u128>()).prop_map(ClientId::from_u128)
}

fn gateway_id() -> impl Strategy<Value = GatewayId> {
    (any::<u128>()).prop_map(GatewayId::from_u128)
}

/// Generates an IPv4 address for the tunnel interface.
///
/// We use the CG-NAT range for IPv4.
fn tunnel_ip4() -> impl Strategy<Value = Ipv4Addr> {
    any::<sample::Index>().prop_map(|idx| {
        let cgnat_block = Ipv4Network::new(Ipv4Addr::new(100, 64, 0, 0), 10).unwrap();

        let mut hosts = cgnat_block.hosts();

        hosts.nth(idx.index(hosts.len())).unwrap()
    })
}

/// Generates an IPv6 address for the tunnel interface.
///
/// TODO: Which subnet do we use here?
fn tunnel_ip6() -> impl Strategy<Value = Ipv6Addr> {
    any::<Ipv6Addr>()
}

fn sim_node_prototype<ID>(
    id: impl Strategy<Value = ID>,
) -> impl Strategy<Value = SimNode<ID, PrivateKey>>
where
    ID: fmt::Debug,
{
    (
        id,
        any::<[u8; 32]>(),
        firezone_relay::proptest::any_ip_stack(), // We are re-using the strategy here because it is exactly what we need although we are generating a node here and not a relay.
        any::<u16>().prop_filter("port must not be 0", |p| *p != 0),
        any::<u16>().prop_filter("port must not be 0", |p| *p != 0),
        tunnel_ip4(),
        tunnel_ip6(),
    )
        .prop_filter_map(
            "must have at least one socket address",
            |(id, key, ip_stack, v4_port, v6_port, tunnel_ip4, tunnel_ip6)| {
                let ip4_socket = ip_stack.as_v4().map(|ip| SocketAddrV4::new(*ip, v4_port));
                let ip6_socket = ip_stack
                    .as_v6()
                    .map(|ip| SocketAddrV6::new(*ip, v6_port, 0, 0));

                Some(SimNode {
                    id,
                    state: PrivateKey(key),
                    ip4_socket,
                    ip6_socket,
                    tunnel_ip4,
                    tunnel_ip6,
                    span: tracing::Span::none(),
                })
            },
        )
}

fn sim_relay_prototype() -> impl Strategy<Value = SimRelay<u64>> {
    (
        any::<u64>(),
        firezone_relay::proptest::dual_ip_stack(), // For this test, our relays always run in dual-stack mode to ensure connectivity!
        any::<u128>(),
    )
        .prop_map(|(seed, ip_stack, id)| SimRelay {
            id: RelayId::from_u128(id),
            state: seed,
            ip_stack,
            span: tracing::Span::none(),
            allocations: HashSet::new(),
            buffer: vec![0u8; (1 << 16) - 1],
        })
}

fn upstream_dns_servers() -> impl Strategy<Value = Vec<DnsServer>> {
    collection::vec(
        any::<IpAddr>().prop_map(|ip| DnsServer::from((ip, 53))),
        0..4,
    )
}

fn system_dns_servers() -> impl Strategy<Value = Vec<IpAddr>> {
    collection::vec(any::<IpAddr>(), 1..4) // Always need at least 1 system DNS server. TODO: Should we test what happens if we don't?
}
