//! The network section: a landing page and the proxy reading under it.
//!
//! What this is NOT: the fleet network view at `/inv/network`, which reads
//! GLPI's v1 collections for the ports and addresses each machine reported.
//! That stays under Inv. These pages are about this machine's own
//! networking: the layer a request passes through on its way to GLPI, rather
//! than anything GLPI collected.
//!
//! The proxy page is first because it answers a question that comes up
//! whenever anyone looks at this install's nginx and finds the word proxy
//! scattered through its config: whether any of it is in force.
use crate::api::{self, NginxProxyReport};
use crate::app::Route;
use crate::components::{FetchError, InfoButton, Panel, StatTile};
use dioxus::prelude::*;
use dioxus_router::Link;

#[component]
pub fn Netw() -> Element {
    rsx! {
        Panel {
            title: "Network".to_string(),
            subtitle: "This machine's own networking, not the fleet's".to_string(),
            info: "What a request meets on the way to GLPI, read from this machine rather \
                   than from GLPI's database. Why this is separate from Inv's Network view: \
                   that one lists the ports and addresses the agents collected from every \
                   machine in the fleet, which is inventory. This section is about the box \
                   the dashboard is running on and the server in front of GLPI on it, which \
                   is why a reading here can disagree with nothing - there is one of each."
                .to_string(),
            ProxyLink {}
        }
    }
}

/// The pointer to the proxy page, carrying enough of the answer to be worth
/// following.
///
/// The same rule the /perf links follow: a bare link is a dead end, because a
/// reader cannot tell whether there is anything behind it. The two counts
/// here are the two tiles on the page, so the click confirms rather than
/// reveals - which is the right trade for a figure that is usually zero.
#[component]
fn ProxyLink() -> Element {
    let report = use_resource(|| async move { api::fetch_nginx_proxy().await });

    rsx! {
        div { class: "flex flex-wrap items-baseline gap-x-3 gap-y-1",
            Link {
                to: Route::NetwProxy {},
                class: "link link-primary font-medium",
                "proxy"
            }
            match &*report.read_unchecked() {
                Some(Ok(r)) if r.config_readable => rsx! {
                    span { class: "text-sm text-base-content/60",
                        "{r.forward} forward \u{b7} {r.reverse} reverse \u{b7} {r.gateways} gateway"
                        if r.gateways != 1 { "s" }
                    }
                },
                // The link still goes. The page it lands on explains why
                // there is nothing to count, which is more use than a
                // summary that quietly says zero for a config nobody read.
                Some(Ok(_)) => rsx! {
                    span { class: "text-sm text-base-content/60", "config not readable" }
                },
                _ => rsx! {},
            }
        }

        p { class: "mt-2 max-w-prose text-sm text-base-content/70",
            "nginx can answer a request from disk, hand it to an application over a gateway \
             protocol, or pass it to another HTTP server and relay the answer back. Only the \
             last is proxying, and this says whether any of it happens here."
        }
    }
}

#[component]
pub fn NetwProxy() -> Element {
    let report = use_resource(|| async move { api::fetch_nginx_proxy().await });

    rsx! {
        div { class: "mb-3 flex flex-wrap items-center gap-x-4 gap-y-2",
            Link { to: Route::Netw {}, class: "link link-primary text-sm", "\u{2190} Netw" }
            InfoButton {
                label: "What is this".to_string(),
                title: "Where a request goes after nginx".to_string(),
                body: "A proxy stands between a client and a server and passes requests on. \
                       WHICH SIDE IT STANDS ON IS THE WHOLE DISTINCTION, and it is what the \
                       two figures below split: a forward proxy acts for the CLIENTS, which \
                       choose to send their traffic through it and name a different \
                       destination in every request; a reverse proxy acts for the SERVERS, \
                       and the client reaching it believes it is the site. Same directive, \
                       proxy_pass, in both cases - which is why the config alone does not \
                       say which, and why the target is read rather than counted: a fixed \
                       target is a server being fronted, and a target built from the \
                       request is one the client chose. Read from the config file the \
                       running master was started with, so this is the server now serving \
                       and not whatever a file elsewhere on disk asks for. Nothing here is \
                       timed and nothing is requested: proxying is a configuration, and \
                       nginx publishes no counter that would report it."
                    .to_string(),
            }
        }

        match &*report.read_unchecked() {
            None => rsx! {
                div { class: "flex items-center gap-2 text-sm text-base-content/60",
                    span { class: "loading loading-spinner loading-sm" }
                    "Reading the config\u{2026}"
                }
            },
            Some(Err(e)) => rsx! { FetchError { error: e.clone() } },
            Some(Ok(r)) => rsx! { ProxyBody { report: r.clone() } },
        }
    }
}

/// Yes, no, or we could not ask.
///
/// Three states and not two, for the reason the backend keeps the field an
/// Option: on a packaged install the binary on the master's command line is
/// not executable by this service, and "absent" would be a claim nobody
/// checked. This app draws that distinction everywhere else.
fn module(present: Option<bool>) -> &'static str {
    match present {
        Some(true) => "in this build",
        Some(false) => "not in this build",
        None => "could not ask the binary",
    }
}

#[component]
fn ProxyBody(report: NginxProxyReport) -> Element {
    let r = report;

    // The config is the only source here, so an unreadable one is not a
    // degraded reading - it is no reading. Every count below would be zero
    // and every zero would be wrong.
    if !r.running || !r.config_readable {
        return rsx! {
            div { class: "alert alert-warning",
                div {
                    div { class: "font-semibold",
                        if !r.running {
                            "No nginx master process is running."
                        } else {
                            "The running config is not readable by this service."
                        }
                    }
                    p { class: "mt-1 text-sm",
                        "Nothing is counted rather than counted as zero. The two figures this \
                         page exists for come from that file and from nowhere else, so an \
                         absence of proxying has not been established - only an absence of a \
                         file to establish it from."
                    }
                    if let Some(p) = r.config_path.as_ref() {
                        pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{p}" }
                    }
                }
            }
        };
    }

    rsx! {
        div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",
            StatTile {
                title: "proxy".to_string(),
                value: r.forward.to_string(),
                desc: "locations proxying for clients".to_string(),
                body: "A forward proxy, the kind a browser or a company network is \
                       configured to send everything through. It acts for the CLIENT: each \
                       request names its own destination, so the target cannot be written \
                       in the config and has to be built from the request - which is what \
                       is counted here, a proxy_pass whose target contains a variable. Two \
                       things are needed besides and are reported under these tiles: a \
                       resolver, because a hostname arriving in a request has to be looked \
                       up while the request waits, and CONNECT for anything over HTTPS, \
                       which no stock nginx can do at all - it is a third-party patch. So a \
                       zero here is nearly always the honest state of an nginx: it is a \
                       capable reverse proxy and a poor forward one, and nobody configures \
                       the second by accident."
                    .to_string(),
            }
            StatTile {
                title: "reversed proxy".to_string(),
                value: r.reverse.to_string(),
                desc: "locations fronting a server".to_string(),
                body: "A reverse proxy, which is what people usually mean by \"nginx as a \
                       proxy\". It acts for the SERVER: the client reaching it believes it \
                       is the site, and nginx passes the request to a fixed target behind \
                       it and relays the answer back. Counted as proxy_pass directives in \
                       the http block whose target is written out rather than built from \
                       the request, so each one is a location being fronted. What this is \
                       NOT: a gateway. fastcgi_pass hands a request to php-fpm and is how \
                       GLPI is served, but php-fpm is part of this install rather than a \
                       server being fronted, and the protocol is not HTTP - so it is \
                       counted under the gateways below and never here. A dashboard that \
                       blurred the two would report every PHP site in the world as a \
                       reverse proxy."
                    .to_string(),
            }
        }

        Panel {
            title: "What passes through".to_string(),
            subtitle: "Every directive in the config that hands a request on".to_string(),
            info: "nginx has six directives that pass a request to something else and only \
                   proxy_pass is proxying; the rest speak a gateway protocol to an \
                   application. They are listed together because the two figures above are \
                   usually zero, and a zero on its own invites the wrong conclusion - that \
                   nothing leaves nginx, which on an install serving PHP is the opposite of \
                   true. The context column is the other half: a pass inside a stream block \
                   forwards TCP or UDP and never sees a request, so it is proxying in a \
                   sense neither tile covers and is counted on its own."
                .to_string(),

            if r.passes.is_empty() {
                p { class: "text-sm text-base-content/70",
                    "Nothing. Every request is answered from disk - no application, no \
                     upstream, no forwarding."
                }
            } else {
                div { class: "overflow-x-auto",
                    table { class: "table table-sm",
                        thead {
                            tr {
                                th { "directive" }
                                th { "target" }
                                th { "context" }
                                th { "kind" }
                            }
                        }
                        tbody {
                            for (i, p) in r.passes.iter().enumerate() {
                                tr { key: "{i}-{p.directive}-{p.target}",
                                    td { class: "font-mono text-xs whitespace-nowrap",
                                        // Per row, because what a reader needs
                                        // is not what the six directives are
                                        // but which of them THIS one is.
                                        div { class: "flex items-center gap-1.5",
                                            span { "{p.directive}" }
                                            InfoButton {
                                                title: p.directive.clone(),
                                                body: p.note.clone(),
                                            }
                                        }
                                    }
                                    td { class: "font-mono text-xs break-all", "{p.target}" }
                                    td { class: "text-xs", "{p.context}" }
                                    td { class: "text-xs",
                                        if p.directive != "proxy_pass" {
                                            "gateway"
                                        } else if p.context == "stream" {
                                            "TCP/UDP forward"
                                        } else if p.dynamic {
                                            "forward proxy"
                                        } else {
                                            "reverse proxy"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Under the table rather than as tiles of their own: each is a
            // fact about what the counts above would need, and none of them
            // is a figure anyone would come to this page to read.
            dl { class: "mt-4 grid gap-x-6 gap-y-2 text-sm sm:grid-cols-2",
                div {
                    dt { class: "flex items-center gap-1.5 text-xs text-base-content/60",
                        span { "resolver" }
                        InfoButton {
                            title: "resolver".to_string(),
                            body: "The DNS server nginx uses to look up a name at request \
                                   time. A reverse proxy rarely needs one - its targets are \
                                   written in the config and resolved when nginx starts. A \
                                   forward proxy cannot work without one, because the host \
                                   it must reach arrives in the request. Its absence is \
                                   therefore the second half of the forward-proxy figure \
                                   above."
                                .to_string(),
                        }
                    }
                    dd { class: "font-mono text-xs",
                        if let Some(res) = r.resolver.as_ref() { "{res}" } else { "not set" }
                    }
                }
                div {
                    dt { class: "flex items-center gap-1.5 text-xs text-base-content/60",
                        span { "upstream blocks" }
                        InfoButton {
                            title: "upstream blocks".to_string(),
                            body: "A named pool of servers a proxy_pass can point at, with \
                                   the balancing, health checks and keepalive that come with \
                                   one. Not required to proxy - a proxy_pass may name a \
                                   single server directly - so this is not a second count of \
                                   the same thing. It says whether what is behind the proxy \
                                   can lose a member and keep serving."
                                .to_string(),
                        }
                    }
                    dd { class: "font-mono text-xs",
                        if r.upstreams.is_empty() { "none" } else { "{r.upstreams.join(\", \")}" }
                    }
                }
                div {
                    dt { class: "flex items-center gap-1.5 text-xs text-base-content/60",
                        span { "proxy module" }
                        InfoButton {
                            title: "the modules this would need".to_string(),
                            body: "Three separate questions, read from the binary's own \
                                   configure line. http_proxy is built into every stock \
                                   nginx unless somebody asked for it to be left out, so a \
                                   zero on the tiles above is a choice and not a limit. \
                                   stream is what makes TCP and UDP forwarding possible at \
                                   all. CONNECT is the one that is genuinely absent from \
                                   stock builds: it is a third-party patch, and without it a \
                                   forward proxy cannot carry HTTPS."
                                .to_string(),
                        }
                    }
                    dd { class: "text-xs",
                        "http_proxy {module(r.proxy_module)} \u{b7} stream \
                         {module(r.stream_module)} \u{b7} CONNECT {module(r.connect_module)}"
                    }
                }
                div {
                    dt { class: "flex items-center gap-1.5 text-xs text-base-content/60",
                        span { "config read" }
                        InfoButton {
                            title: "the file these counts come from".to_string(),
                            body: "The config the RUNNING master was started with, taken \
                                   from its own command line rather than from the \
                                   conventional path - on this machine the two are not the \
                                   same, because the running one is a Nix store path. \
                                   Include lines are named and NOT followed: a pass inside \
                                   an included file would not be counted, and resolving a \
                                   glob against a directory this service may not read would \
                                   turn a partial answer into one presented as whole."
                                .to_string(),
                        }
                    }
                    dd { class: "font-mono text-xs break-all",
                        if let Some(p) = r.config_path.as_ref() { "{p}" } else { "unknown" }
                    }
                }
            }

            if !r.includes.is_empty() {
                p { class: "mt-3 text-xs text-base-content/60",
                    if r.includes.len() == 1 {
                        "One include line was not followed: "
                    } else {
                        "{r.includes.len()} include lines were not followed: "
                    }
                    span { class: "font-mono break-all", "{r.includes.join(\", \")}" }
                }
            }
        }

        div { class: "mt-5",
            Panel {
                title: "Proxy settings with nothing to apply to".to_string(),
                subtitle: "In the config, waiting for a proxy_pass".to_string(),
                info: "The reason this panel exists: anyone grepping this install's config \
                       for \"proxy\" gets hits, and concludes something is proxied. All of \
                       these are emitted by the NixOS nginx module or copied from nginx's \
                       own websocket guide rather than written by someone configuring a \
                       proxy, and each does exactly nothing until a proxy_pass exists. \
                       Present and used are therefore two columns and not one - a setting in \
                       the file and a setting in force are different facts, and only the \
                       second changes what a request does."
                    .to_string(),

                ul { class: "space-y-2",
                    for s in r.support.iter() {
                        li { key: "{s.what}", class: "flex flex-wrap items-baseline gap-x-2",
                            span { class: "font-mono text-xs", "{s.what}" }
                            InfoButton { title: s.what.clone(), body: s.note.clone() }
                            span { class: "text-xs text-base-content/60",
                                if !s.present {
                                    "not in the config"
                                } else if s.used {
                                    "in the config, and in force"
                                } else {
                                    "in the config, applies to nothing"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
