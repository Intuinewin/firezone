"use client";

import CustomerLogos from "@/components/CustomerLogos";
import Toggle from "@/components/Toggle";
import { HiCheck } from "react-icons/hi2";
import Link from "next/link";
import PlanTable from "./plan_table";
import { useState } from "react";

export default function _Page() {
  let [annual, setAnnual] = useState(true);
  let teamPrice: string;

  return (
    <>
      <section className="bg-neutral-100">
        <div className="py-8 px-4 mx-auto max-w-screen-xl lg:py-16 lg:px-6">
          <div className="mx-auto max-w-screen-md sm:text-center">
            <h1 className="text-center justify-center mb-4 text-3xl font-extrabold leading-none tracking-tight text-neutral-900 sm:text-6xl">
              Plans & Pricing
            </h1>
            <h2 className="text-center justify-center mb-8 tracking-tight text-neutral-900 md:mb-12 sm:text-2xl text-xl">
              Pick a plan that best suits your needs. No credit card required to
              sign up.
            </h2>
          </div>
        </div>
      </section>
      <section className="bg-neutral-100 border-t border-neutral-200 pb-14">
        <div className="flex justify-center mt-12">
          <span
            className={
              (annual ? "text-neutral-600 " : "text-neutral-900 ") +
              "font-medium me-3 text-lg"
            }
          >
            MONTHLY
          </span>
          <Toggle checked={annual} onChange={setAnnual} />
          <span
            className={
              (annual ? "text-neutral-900 " : "text-neutral-600 ") +
              "font-medium ms-3 text-lg"
            }
          >
            ANNUAL
            <span className="text-sm text-neutral-700 text-primary-450">
              {" "}
              (Save 17%)
            </span>
          </span>
        </div>
        <div className="mx-auto max-w-screen-2xl md:grid md:grid-cols-3 pt-14 md:gap-4 px-4">
          <div className="p-8 bg-neutral-50 rounded border-2 border-neutral-200 mb-4">
            <h3 className="mb-4 text-2xl tracking-tight font-semibold text-primary-450">
              Starter
            </h3>
            <p className="mb-8">
              Secure remote access for individuals and small groups
            </p>
            <h2 className="mb-16 text-2xl sm:text-4xl tracking-tight font-semibold text-neutral-900">
              Free
            </h2>
            <div className="mb-24 w-full text-center">
              <Link href="https://app.firezone.dev/sign_up">
                <button
                  type="button"
                  className="w-64 text-lg px-5 py-2.5 md:w-44 md:text-sm md:px-3 md:py-2.5 lg:w-64 lg:text-lg lg:px-5 lg:py-2.5 border border-1 border-primary-450 hover:border-2 hover:font-bold font-semibold tracking-tight rounded shadow-lg text-primary-450 duration-0 hover:scale-105 transition transform"
                >
                  Sign up
                </button>
              </Link>
            </div>
            <ul role="list" className="font-medium space-y-2">
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Up to 6 users
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Access your homelab or VPC from anywhere
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Native clients for Windows, Linux, macOS, iOS, Android
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Authenticate via email or OpenID Connect (OIDC)
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Load balancing and automatic failover
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  No firewall configuration required
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Community Support
                </span>
              </li>
            </ul>
          </div>
          <div className="p-8 bg-neutral-50 rounded border-2 border-neutral-200 mb-4">
            <h3 className="mb-4 text-2xl tracking-tight font-semibold text-primary-450">
              Team
            </h3>
            <p className="mb-8">
              Zero trust network access for teams and organizations
            </p>
            <h2 className="mb-16 text-2xl sm:text-4xl tracking-tight font-semibold text-neutral-900">
              {annual && (
                <>
                  <span className="line-through">$5</span>
                  <span className="text-primary-450">$4.16</span>
                </>
              )}
              {!annual && <span>$5</span>}
              <span className="h-full">
                <span className="text-xs text-neutral-700 inline-block align-bottom ml-1 mb-1">
                  {" "}
                  per user / month
                </span>
              </span>
            </h2>
            <div className="mb-16 w-full text-center">
              <Link href="https://app.firezone.dev/sign_up">
                <button
                  type="button"
                  className="w-64 text-lg px-5 py-2.5 md:w-44 md:text-sm md:px-3 md:py-2.5 lg:w-64 lg:text-lg lg:px-5 lg:py-2.5 border border-1 border-primary-450 hover:border-2 hover:font-bold font-semibold tracking-tight rounded shadow-lg text-primary-450 duration-0 hover:scale-105 transition transform"
                >
                  Sign up
                </button>
              </Link>
            </div>
            <p className="mb-2">
              <strong>Everything in Starter, plus:</strong>
            </p>
            <ul role="list" className="font-medium space-y-2">
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Up to 100 users
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Resource access logs
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Port and protocol traffic restrictions
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Customize your account slug
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Faster relay network
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5 text-neutral-900" />
                <span className="leading-tight text-neutral-900 ">
                  Priority email support
                </span>
              </li>
            </ul>
          </div>
          <div className="p-8 bg-neutral-900 text-neutral-50 rounded shadow border border-primary-450 mb-4">
            <div className="mb-4 flex items-center justify-between">
              <h3 className="text-2xl tracking-tight font-semibold text-primary-450">
                Enterprise
              </h3>
              <span className="font-semibold uppercase text-xs rounded bg-neutral-50 text-neutral-800 px-1 py-0.5">
                30-day trial
              </span>
            </div>
            <p className="mb-8 font-semibold">
              Compliance-ready security for large organizations
            </p>
            <h2 className="mb-16 text-2xl sm:text-4xl tracking-tight font-semibold">
              Contact us
            </h2>
            <div className="mb-16 w-full text-center">
              <Link href="/contact/sales">
                <button
                  type="button"
                  className="w-64 text-lg px-5 py-2.5 md:w-44 md:text-sm md:px-3 md:py-2.5 lg:w-64 lg:text-lg lg:px-5 lg:py-2.5 text-white font-semibold hover:font-bold tracking-tight transition transform duration-0 hover:scale-105 rounded bg-primary-450 shadow-lg shadow-primary-700"
                >
                  Request a demo
                </button>
              </Link>
            </div>
            <p className="mb-2">
              <strong>Everything in Team, plus:</strong>
            </p>
            <ul role="list" className="font-medium space-y-2">
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5" />
                <span className="leading-tight">Unlimited users</span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5" />
                <span className="leading-tight">
                  Directory sync for Google, Entra ID, and Okta
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5" />
                <span className="leading-tight">
                  <span className="font-semibold text-primary-450">
                    Unthrottled
                  </span>{" "}
                  relay network
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5" />
                <span className="leading-tight">
                  Dedicated Slack support channel
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5" />
                <span className="leading-tight">
                  <span className="font-semibold text-primary-450">99.99%</span>{" "}
                  uptime SLA
                </span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5" />
                <span className="leading-tight">Roadmap acceleration</span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5" />
                <span className="leading-tight">White-glove onboarding</span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5" />
                <span className="leading-tight">Annual invoicing</span>
              </li>
              <li className="flex space-x-2.5">
                <HiCheck className="flex-shrink-0 w-5 h-5" />
                <span className="leading-tight">
                  Pentested &{" "}
                  <span className="font-semibold text-primary-450">SOC 2</span>{" "}
                  compliant
                </span>
              </li>
            </ul>
          </div>
        </div>
      </section>
      <section className="py-24 bg-gradient-to-b to-white from-neutral-100 via-primary-100">
        <CustomerLogos />
      </section>
      <section className="bg-white py-14">
        <div className="mb-14 mx-auto max-w-screen-lg px-3">
          <h2 className="mb-14 justify-center text-4xl font-bold text-neutral-900">
            Compare plans
          </h2>
          <PlanTable />
        </div>
      </section>
      <section className="bg-neutral-100 border-t border-neutral-200 p-14">
        <div className="mx-auto max-w-screen-sm">
          <h2 className="mb-14 justify-center text-4xl font-bold text-neutral-900">
            FAQ
          </h2>

          <div className="px-4 w-full mb-14">
            <ol className="list-decimal">
              <li>
                <Link
                  href="#how-long"
                  className="hover:underline text-accent-500"
                >
                  How long does it take to set up Firezone?
                </Link>
              </li>
              <li>
                <Link
                  href="#rip-replace"
                  className="hover:underline text-accent-500"
                >
                  Do I need to rip and replace my current VPN to use Firezone?
                </Link>
              </li>
              <li>
                <Link href="#data" className="hover:underline text-accent-500">
                  What happens to my data with Firezone enabled?
                </Link>
              </li>
              <li>
                <Link
                  href="#change-plan"
                  className="hover:underline text-accent-500"
                >
                  How do I cancel or change my plan?
                </Link>
              </li>
              <li>
                <Link
                  href="#when-billed"
                  className="hover:underline text-accent-500"
                >
                  When will I be billed?
                </Link>
              </li>
              <li>
                <Link
                  href="#payment-methods"
                  className="hover:underline text-accent-500"
                >
                  What payment methods are available?
                </Link>
              </li>
              <li>
                <Link
                  href="#special-pricing"
                  className="hover:underline text-accent-500"
                >
                  Do you offer special pricing for nonprofits and educational
                  institutions?
                </Link>
              </li>
              <li>
                <Link
                  href="#special-pricing"
                  className="hover:underline text-accent-500"
                >
                  Other than using Firezone, is there anything I can do to
                  improve my cybersecurity?
                </Link>
              </li>
            </ol>
          </div>

          <a id="how-long" className="pt-8"></a>
          <blockquote className="font-semibold text-md p-2 my-2 border-s-4 border-neutral-300">
            <p>How long does it take to set up Firezone?</p>
          </blockquote>
          <p className="mb-8">
            Firezone can be set up in{" "}
            <Link
              href="/kb/quickstart"
              className="hover:underline text-accent-500"
            >
              less than 10 minutes
            </Link>
            , and Gateways can be added by running a simple Docker command.{" "}
            <Link href="/kb" className="hover:underline text-accent-500">
              Visit our docs
            </Link>{" "}
            for more information and step by step instructions.
          </p>

          <a id="rip-replace" className="pt-8"></a>
          <blockquote className="font-semibold text-md p-2 my-2 border-s-4 border-neutral-300">
            <p>Do I need to rip and replace my current VPN with Firezone?</p>
          </blockquote>
          <p className="mb-8">
            No. As long they're set up to access different resources, you can
            run Firezone alongside your existing remote access solutions, and
            switch over whenever you’re ready. There’s no need for any downtime
            or unnecessary disruptions.
          </p>

          <a id="data" className="pt-8"></a>
          <blockquote className="font-semibold text-md p-2 my-2 border-s-4 border-neutral-300">
            <p>What happens to my data with Firezone enabled?</p>
          </blockquote>
          <p className="mb-8">
            Network traffic is always end-to-end encrypted, and by default,
            routes directly to Gateways running on your infrastructure. In rare
            circumstances, encrypted traffic can pass through our global relay
            network if a direct connection cannot be established. Firezone can
            never decrypt the contents of your traffic.
          </p>

          <a id="change-plan" className="pt-8"></a>
          <blockquote className="font-semibold text-md p-2 my-2 border-s-4 border-neutral-300">
            <p>How do I cancel or change my plan?</p>
          </blockquote>
          <p className="mb-8">
            Please{" "}
            <Link
              href="mailto:support@firezone.dev"
              className="hover:underline text-accent-500"
            >
              contact support
            </Link>{" "}
            if you would like to change your plan or terminate your account.
          </p>

          <a id="when-billed" className="pt-8"></a>
          <blockquote className="font-semibold text-md p-2 my-2 border-s-4 border-neutral-300">
            <p>When will I be billed?</p>
          </blockquote>
          <p className="mb-8">
            The Team plan is billed monthly on the same day you start service
            until canceled. Enterprise plans are billed annually.
          </p>

          <a id="payment-methods" className="pt-8"></a>
          <blockquote className="font-semibold text-md p-2 my-2 border-s-4 border-neutral-300">
            <p>What payment methods are available?</p>
          </blockquote>
          <p className="mb-8">
            The Starter plan is free and does not require a credit card to get
            started. Team and Enterprise plans can be paid via credit card, ACH,
            or wire transfer.
          </p>

          <a id="special-pricing" className="pt-8"></a>
          <blockquote className="font-semibold text-md p-2 my-2 border-s-4 border-neutral-300">
            <p>
              Do you offer special pricing for nonprofits and educational
              institutions?
            </p>
          </blockquote>
          <p className="mb-8">
            Yes. Not-for-profit organizations and educational institutions are
            eligible for a 50% discount.{" "}
            <Link
              href="/contact/sales"
              className="hover:underline text-accent-500"
            >
              Contact sales
            </Link>{" "}
            to request the discount.
          </p>
        </div>
      </section>

      <section className="bg-neutral-100 border-t border-neutral-200 p-14">
        <div className="mx-auto max-w-screen-xl md:grid md:grid-cols-2">
          <div>
            <h2 className="w-full justify-center mb-8 text-2xl md:text-3xl font-semibold text-neutral-900">
              The WireGuard® solution for Enterprise.
            </h2>
          </div>
          <div className="mb-14 w-full text-center">
            <Link href="/contact/sales">
              <button
                type="button"
                className="w-64 text-white tracking-tight rounded duration-0 hover:scale-105 transition transform shadow-lg text-lg px-5 py-2.5 bg-primary-450 font-semibold hover:font-bold"
              >
                Request a demo
              </button>
            </Link>
          </div>
        </div>
      </section>
    </>
  );
}
