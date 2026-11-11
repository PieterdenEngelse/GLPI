//! The inventory section: a landing page and four views beneath it.
//!
//! One module rather than five files. The four pages share a sub-header and
//! are at the same stage, so keeping them together makes the family obvious
//! and the shared nav one definition instead of four copies.
//!
//! All four are fleet-wide: one pass over the estate rather than a machine
//! picker. That is a deliberate consequence of where the data comes from.
//! v2.3 exposes components, volumes and software installs only beneath a
//! single asset id, which would make a fleet view cost one request per
//! machine per kind; the backend reads GLPI's v1 collections instead, which
//! answer for the whole estate in one call each. The reasoning, and the
//! verification behind it, is in backend/src/glpi/inventory.rs.
//!
//! Sizes are GLPI's own units - megabytes for disks and memory, megahertz for
//! processors - converted for display here and nowhere else, so what the
//! backend serves still matches what GLPI stores.
use crate::api::{self, HardwareRow, NetworkRow, SoftwareRow};
use crate::app::Route;
use crate::components::{InfoLink, InfoSection, Panel, RefreshControl, StatTile};
use dioxus::prelude::*;
use dioxus_router::{use_route, Link};

/// How often the three fleet views refetch, matching the other pages.
const REFRESH_MS: u32 = 5 * 60 * 1000;
const REFRESH_LABEL: &str = "5m";

/// Megabytes as GLPI stores them, in units a reader can hold in their head.
///
/// Decimal GB, not GiB: GLPI's own screens report decimal, and a dashboard
/// disagreeing with the system it reads is worse than either convention.
fn mb(size: i64) -> String {
    match size {
        s if s <= 0 => "-".to_string(),
        s if s < 1024 => format!("{s} MB"),
        s if s < 1024 * 1024 => format!("{:.0} GB", s as f64 / 1024.0),
        s => format!("{:.1} TB", s as f64 / (1024.0 * 1024.0)),
    }
}

/// Megahertz as GHz, for a processor's rated speed.
fn mhz(freq: i64) -> String {
    format!("{:.1} GHz", freq as f64 / 1000.0)
}

/// The state every one of these pages shows while its first fetch is in
/// flight. Three copies of six lines is the thing this avoids.
#[component]
fn Loading() -> Element {
    rsx! {
        div { class: "flex items-center gap-2 text-base-content/60",
            span { class: "loading loading-spinner loading-sm" }
            "Loading…"
        }
    }
}

/// An upstream that could not be reached.
///
/// Its own component for the reason stated in CLAUDE.md: this must never
/// degrade into an empty table. An empty fleet and an unreachable GLPI mean
/// opposite things, and the error carries GLPI's own message through the
/// backend's 502.
#[component]
fn Unreachable(what: String, error: String) -> Element {
    rsx! {
        div { class: "alert alert-error",
            div {
                div { class: "font-semibold", "Could not read {what}." }
                pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{error}" }
            }
        }
    }
}

/// The sub-header. Four links, the active one lit.
///
/// Active state comes from `use_route`, not a signal the links write to: a
/// signal only knows about clicks, so a pasted URL or the back button would
/// leave the wrong link lit - the same reasoning as the main NavTabs.
#[component]
fn InvNav() -> Element {
    let current = use_route::<Route>();
    let is = |r: &Route| std::mem::discriminant(&current) == std::mem::discriminant(r);

    let links = [
        (Route::InvHardware {}, "hardware"),
        (Route::InvSoftware {}, "software"),
        (Route::InvNetwork {}, "network"),
        (Route::InvRefused {}, "refused"),
    ];

    rsx! {
        nav {
            role: "tablist",
            class: "tabs tabs-box tabs-sm mb-5 w-full justify-start",
            for (route, label) in links {
                Link {
                    key: "{label}",
                    class: if is(&route) { "tab tab-active" } else { "tab" },
                    to: route,
                    "{label}"
                }
            }
        }
    }
}

#[component]
pub fn Inv() -> Element {
    rsx! {
        InvNav {}
        Panel {
            title: "Inventory".to_string(),
            subtitle: "Four fleet-wide views of what the agents collected".to_string(),
            info: "What the inventory agents collect, split by the kind of thing it is."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why this is separate from Agents and Computers",
                    "Those two pages answer whether the collecting is working - which agents \
                     report, and which machines no agent speaks for. These four are about what \
                     was collected. A fleet can be perfectly healthy by the first measure and \
                     still hold nothing you can act on.",
                ),
                InfoSection::new(
                    "Where these four read from",
                    "GLPI's older v1 API, unlike every other page here. Not a preference: the \
                     current API exposes components, volumes and software installs only one \
                     machine at a time, and exposes network ports and IP addresses at no \
                     address at all. v1 answers each of them for the whole estate in a single \
                     call. It is the same account with the same profile and the same entity \
                     scope, so these pages show exactly what the rest of the app can see.",
                ),
                InfoSection::new(
                    "Refused is still the odd one out",
                    "The other three are read from GLPI's API. Refused equipment is in neither \
                     API and is read from the database directly, which is why that page says \
                     so and explains the trade on its own terms.",
                ),
            ],

            p { class: "max-w-prose text-sm text-base-content/80",
                "Pick a view above. Hardware is what is inside each machine, Software is what \
                 is installed across the fleet, Network is the ports and addresses each \
                 machine reported, and Refused is what GLPI threw away."
            }
        }
    }
}

#[component]
pub fn InvHardware() -> Element {
    let mut rows = use_resource(|| async move { api::fetch_hardware().await });

    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(REFRESH_MS).await;
            rows.restart();
        }
    });

    rsx! {
        InvNav {}
        match &*rows.read_unchecked() {
            Some(Err(e)) => rsx! {
                Unreachable { what: "hardware".to_string(), error: e.clone() }
            },
            Some(Ok(list)) => {
                let bare = list.iter().filter(|r| r.empty).count();
                let storage: i64 = list
                    .iter()
                    .flat_map(|r| r.disks.iter())
                    .filter_map(|d| d.totalsize)
                    .sum();
                rsx! {
                    div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",
                        StatTile {
                            title: "Machines".to_string(),
                            value: list.len().to_string(),
                            desc: "with an asset record".to_string(),
                            body: "Every computer this account can see. The same fleet the \
                                   Computers page counts, so the two always agree."
                                .to_string(),
                            links: vec![InfoLink::new("Computers", "front/computer.php")],
                        }
                        StatTile {
                            title: "Nothing collected".to_string(),
                            value: bare.to_string(),
                            alarm: bare > 0,
                            desc: "asset exists, inventory never landed".to_string(),
                            body: "Machines with an asset record but no processor, memory, \
                                   disk or operating system recorded against them. That \
                                   usually means the record was created by hand, or that an \
                                   agent registered and then never successfully submitted; \
                                   either way GLPI lists the machine beside every other, with \
                                   nothing to say that everything known about it was typed \
                                   in. It is not the same as a machine missing one kind of \
                                   detail - a laptop that reported its disks but no memory \
                                   modules is not counted here, only machines where none of \
                                   the four arrived."
                                .to_string(),
                            links: vec![InfoLink::new("Agents", "front/agent.php")],
                        }
                        StatTile {
                            title: "Storage".to_string(),
                            value: mb(storage),
                            desc: "across every reported volume".to_string(),
                            body: "The total size of every filesystem the agents reported, \
                                   added up across the fleet. That is every volume GLPI holds \
                                   for these machines, including pseudo-filesystems the agent \
                                   reported with a size of zero: those add nothing to the \
                                   total but do appear in the per-machine volume counts \
                                   below."
                                .to_string(),
                        }
                    }

                    Panel {
                        title: "Hardware".to_string(),
                        subtitle: "Machines with nothing collected first".to_string(),
                        info: "What the agents found inside each machine - processors, \
                               memory, disks and the operating system on them."
                            .to_string(),
                        info_sections: vec![
                            InfoSection::new(
                                "The columns",
                                "Processor is the model the agent reported, with its core and \
                                 thread count and rated speed. Memory is the installed total \
                                 and how many modules make it up. Storage is the sum of every \
                                 volume and how full they are together. OS is the operating \
                                 system with its kernel version.",
                            ),
                            InfoSection::new(
                                "Where it comes from",
                                "GLPI's v1 collections, one call each for the whole fleet. \
                                 The current API serves the same facts only one machine at a \
                                 time, so a fleet view there would cost ten requests per \
                                 machine instead of ten in total.",
                            ),
                            InfoSection::new(
                                "Why a figure can be missing",
                                "A blank cell means the agent did not report that detail, not \
                                 that the machine lacks it. Virtual machines routinely report \
                                 no memory modules, and an architecture is often absent even \
                                 where the operating system is known.",
                            ),
                        ],
                        info_links: vec![InfoLink::new("Computers", "front/computer.php")],
                        actions: rsx! {
                            RefreshControl {
                                interval_label: REFRESH_LABEL.to_string(),
                                on_refresh: move |_| rows.restart(),
                            }
                        },

                        if list.is_empty() {
                            div { class: "text-base-content/60",
                                "No computers in GLPI yet. That is different from an error: "
                                "nothing has been inventoried."
                            }
                        } else {
                            HardwareTable { rows: list.clone() }
                        }
                    }
                }
            },
            None => rsx! { Loading {} },
        }
    }
}

#[component]
fn HardwareTable(rows: Vec<HardwareRow>) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "table table-sm",
                thead {
                    tr {
                        th { "Computer" }
                        th { "Operating system" }
                        th { "Processor" }
                        th { "Memory" }
                        th { "Storage" }
                    }
                }
                tbody {
                    for r in rows.iter() {
                        tr { class: if r.empty { "bg-warning/5" } else { "" },
                            td {
                                div { class: "font-medium",
                                    "{r.computer.clone().unwrap_or_else(|| \"(unnamed)\".into())}"
                                }
                                if r.empty {
                                    div { class: "text-xs text-warning", "nothing collected" }
                                }
                            }
                            HardwareOs { row: r.clone() }
                            HardwareCpu { row: r.clone() }
                            HardwareMemory { row: r.clone() }
                            HardwareStorage { row: r.clone() }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn HardwareOs(row: HardwareRow) -> Element {
    rsx! {
        td {
            match &row.operatingsystem {
                Some(os) => rsx! {
                    div { class: "text-base-content/80",
                        "{os.name.clone().unwrap_or_else(|| \"(unknown)\".into())}"
                    }
                    // Kernel and architecture on one sub-line: both are
                    // qualifiers on the name above rather than facts of
                    // their own, and separate rows for each made the table
                    // three lines tall for every machine.
                    if os.kernel_version.is_some() || os.architecture.is_some() {
                        div { class: "text-xs text-base-content/60",
                            "{os.kernel_version.clone().unwrap_or_default()} "
                            "{os.architecture.clone().unwrap_or_default()}"
                        }
                    }
                },
                None => rsx! { span { class: "text-base-content/40", "-" } },
            }
        }
    }
}

#[component]
fn HardwareCpu(row: HardwareRow) -> Element {
    rsx! {
        td {
            if row.processors.is_empty() {
                span { class: "text-base-content/40", "-" }
            } else {
                for p in row.processors.iter() {
                    div {
                        div { class: "text-base-content/80",
                            "{p.designation.clone().unwrap_or_else(|| \"(unknown)\".into())}"
                        }
                        div { class: "text-xs text-base-content/60",
                            if let Some(c) = p.nbcores {
                                "{c}c"
                            }
                            if let Some(t) = p.nbthreads {
                                "/{t}t"
                            }
                            if let Some(f) = p.frequency {
                                " @ {mhz(f)}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn HardwareMemory(row: HardwareRow) -> Element {
    // Modules can differ in size and type within one machine, so the sub-line
    // names the distinct kinds rather than claiming they are all alike.
    let kinds: Vec<String> = {
        let mut seen: Vec<String> = Vec::new();
        for m in row.memory.iter() {
            if let Some(d) = m.designation.clone() {
                if !seen.contains(&d) {
                    seen.push(d);
                }
            }
        }
        seen
    };

    rsx! {
        td { class: "whitespace-nowrap",
            if row.memory_mb > 0 {
                div { class: "text-base-content/80", "{mb(row.memory_mb)}" }
            } else {
                span { class: "text-base-content/40", "-" }
            }
            if !row.memory.is_empty() {
                div { class: "text-xs text-base-content/60",
                    "{row.memory.len()} module"
                    if row.memory.len() != 1 {
                        "s"
                    }
                    if !kinds.is_empty() {
                        " · {kinds.join(\", \")}"
                    }
                }
            }
        }
    }
}

#[component]
fn HardwareStorage(row: HardwareRow) -> Element {
    // Only volumes with a real size. efivarfs, tmpfs and the rest report
    // zero, and averaging them in would drag every machine's usage toward
    // nothing while adding rows that say the same thing.
    let real: Vec<&api::Disk> = row
        .disks
        .iter()
        .filter(|d| d.totalsize.unwrap_or(0) > 0)
        .collect();
    let total: i64 = real.iter().filter_map(|d| d.totalsize).sum();
    let free: i64 = real.iter().filter_map(|d| d.freesize).sum();
    let used_percent = (total > 0).then(|| ((total - free) * 100) / total);

    rsx! {
        td { class: "whitespace-nowrap",
            if total > 0 {
                div { class: "text-base-content/80",
                    "{mb(total)}"
                    if let Some(p) = used_percent {
                        span { class: if p >= 90 { "text-error" } else { "text-base-content/60" },
                            " · {p}% used"
                        }
                    }
                }
            } else {
                span { class: "text-base-content/40", "-" }
            }
            if !row.disks.is_empty() {
                div {
                    class: "text-xs text-base-content/60",
                    title: "{row.disks.iter().map(|d| d.mountpoint.clone().or_else(|| d.name.clone()).unwrap_or_default()).collect::<Vec<_>>().join(\", \")}",
                    "{real.len()} volume"
                    if real.len() != 1 {
                        "s"
                    }
                    // Pseudo-filesystems are counted but not sized, and
                    // saying so is cheaper than leaving the two numbers to
                    // disagree silently.
                    if row.disks.len() != real.len() {
                        " (+{row.disks.len() - real.len()} unsized)"
                    }
                }
            }
        }
    }
}

/// Rows rendered before the table stops and asks for a filter.
///
/// Not a limit on what was fetched - the count above the table is always the
/// true total. A single NixOS machine holds 1330 titles and the fleet figure
/// grows from there; past a few hundred rows the browser spends longer
/// laying out the table than the backend spent collecting it, and nobody
/// reads row 900 anyway.
const SOFTWARE_ROW_CAP: usize = 200;

#[component]
pub fn InvSoftware() -> Element {
    let mut report = use_resource(|| async move { api::fetch_software().await });
    let mut query = use_signal(String::new);

    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(REFRESH_MS).await;
            report.restart();
        }
    });

    rsx! {
        InvNav {}
        match &*report.read_unchecked() {
            Some(Err(e)) => rsx! {
                Unreachable { what: "software".to_string(), error: e.clone() }
            },
            Some(Ok(r)) => {
                let q = query().trim().to_lowercase();
                let matched: Vec<SoftwareRow> = r
                    .rows
                    .iter()
                    .filter(|row| {
                        q.is_empty()
                            || row
                                .name
                                .as_deref()
                                .is_some_and(|n| n.to_lowercase().contains(&q))
                    })
                    .cloned()
                    .collect();
                let spread = r.rows.iter().filter(|row| row.versions.len() > 1).count();

                rsx! {
                    div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",
                        StatTile {
                            title: "Titles".to_string(),
                            value: r.rows.len().to_string(),
                            desc: "distinct software packages".to_string(),
                            body: "Distinct software titles installed somewhere in the fleet. \
                                   A title installed at four versions still counts once."
                                .to_string(),
                            links: vec![InfoLink::new("Software", "front/software.php")],
                        }
                        StatTile {
                            title: "Installations".to_string(),
                            value: r.installs.to_string(),
                            desc: "across {r.computers_covered} machine(s)".to_string(),
                            body: "One per installed version per machine. Higher than the \
                                   title count whenever a machine carries more than one \
                                   version of the same software. It can look enormous: on a \
                                   package-managed system every library is a title in its own \
                                   right, so a single Linux machine commonly reports well \
                                   over a thousand. That is the agent working correctly, not \
                                   a fault."
                                .to_string(),
                        }
                        StatTile {
                            title: "Multiple versions".to_string(),
                            value: spread.to_string(),
                            desc: "titles not on one version".to_string(),
                            body: "Titles present at more than one version across the fleet - \
                                   the usual shape of a rollout that was started and not \
                                   finished. Not automatically a problem: several versions of \
                                   a shared library installed side by side is normal on some \
                                   systems and deliberate on others. This counts the cases \
                                   worth a look, it does not judge them."
                                .to_string(),
                        }
                    }

                    Panel {
                        title: "Software".to_string(),
                        subtitle: "Most widely installed first".to_string(),
                        info: "Every software title the agents reported, and how far across \
                               the fleet each one reaches. Machines is how many distinct \
                               computers carry the title; Installations counts each installed \
                               version separately, so it is the larger number when one \
                               machine holds two versions; Versions lists the distinct \
                               version strings found. This needed the older API: the current \
                               one reports an installation by its version alone - \"1.33.7\", \
                               with no title attached - and serves the version-to-title \
                               mapping only one title at a time, about 1330 requests on this \
                               install, where the v1 collection returns the whole mapping in \
                               one call."
                            .to_string(),
                        info_links: vec![InfoLink::new("Software", "front/software.php")],
                        actions: rsx! {
                            RefreshControl {
                                interval_label: REFRESH_LABEL.to_string(),
                                on_refresh: move |_| report.restart(),
                            }
                        },

                        if r.rows.is_empty() {
                            div { class: "text-base-content/60",
                                "No software recorded. That is different from an error: no "
                                "agent has reported an installed package."
                            }
                        } else {
                            div { class: "mb-3 flex flex-wrap items-center gap-3",
                                input {
                                    class: "input input-sm w-56",
                                    r#type: "search",
                                    placeholder: "Filter by name…",
                                    value: "{query}",
                                    oninput: move |e| query.set(e.value()),
                                }
                                span { class: "text-xs text-base-content/60",
                                    "{matched.len()} of {r.rows.len()} titles"
                                    if matched.len() > SOFTWARE_ROW_CAP {
                                        ", showing the first {SOFTWARE_ROW_CAP}"
                                    }
                                }
                            }

                            if matched.is_empty() {
                                div { class: "text-base-content/60",
                                    "No title matches that filter."
                                }
                            } else {
                                SoftwareTable {
                                    rows: matched.iter().take(SOFTWARE_ROW_CAP).cloned().collect::<Vec<_>>(),
                                }
                            }
                        }
                    }
                }
            },
            None => rsx! { Loading {} },
        }
    }
}

#[component]
fn SoftwareTable(rows: Vec<SoftwareRow>) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "table table-sm",
                thead {
                    tr {
                        th { "Software" }
                        th { "Versions" }
                        th { class: "text-right", "Machines" }
                        th { class: "text-right", "Installations" }
                    }
                }
                tbody {
                    for r in rows.iter() {
                        tr {
                            td { class: "font-medium", "{r.label()}" }
                            td {
                                div { class: "font-mono text-xs text-base-content/70",
                                    "{r.versions.join(\", \")}"
                                }
                                if r.versions.len() > 1 {
                                    div { class: "text-xs text-warning",
                                        "{r.versions.len()} versions"
                                    }
                                }
                            }
                            td { class: "text-right tabular-nums", "{r.computers}" }
                            td { class: "text-right tabular-nums text-base-content/70",
                                "{r.installs}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn InvNetwork() -> Element {
    let mut rows = use_resource(|| async move { api::fetch_network().await });

    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(REFRESH_MS).await;
            rows.restart();
        }
    });

    rsx! {
        InvNav {}
        match &*rows.read_unchecked() {
            Some(Err(e)) => rsx! {
                Unreachable { what: "the network inventory".to_string(), error: e.clone() }
            },
            Some(Ok(list)) => {
                let ports: usize = list.iter().map(|r| r.ports.len()).sum();
                let addressed = list
                    .iter()
                    .flat_map(|r| r.ports.iter())
                    .filter(|p| !p.ips.is_empty())
                    .count();
                let bare = list.iter().filter(|r| r.empty).count();

                rsx! {
                    div { class: "stats stats-vertical sm:stats-horizontal mb-5 w-full border border-base-300 bg-base-100 shadow-sm",
                        StatTile {
                            title: "Ports".to_string(),
                            value: ports.to_string(),
                            desc: "network interfaces reported".to_string(),
                            body: "Every network interface the agents reported, loopback \
                                   included - GLPI records it like any other."
                                .to_string(),
                            links: vec![InfoLink::new("Computers", "front/computer.php")],
                        }
                        StatTile {
                            title: "With an address".to_string(),
                            value: addressed.to_string(),
                            desc: "of {ports} carry an IP".to_string(),
                            body: "Ports with at least one address recorded against them, v4 \
                                   or v6. A port with none is usually simply down."
                                .to_string(),
                        }
                        StatTile {
                            title: "No port recorded".to_string(),
                            value: bare.to_string(),
                            alarm: bare > 0,
                            desc: "machines with no interface at all".to_string(),
                            body: "Machines where no network interface was reported. A \
                                   networked machine that reports none has usually never had \
                                   a successful inventory. Essentially every real machine has \
                                   at least a loopback interface, so none at all points at \
                                   the asset record rather than the network - typically one \
                                   created by hand, or one whose agent has never submitted \
                                   successfully."
                                .to_string(),
                        }
                    }

                    Panel {
                        title: "Network".to_string(),
                        subtitle: "One row per port, machines without any first".to_string(),
                        info: "The network interfaces each machine reported, with their \
                               hardware addresses and the IP addresses on them."
                            .to_string(),
                        info_sections: vec![
                            InfoSection::new(
                                "The columns",
                                "Port is the interface name as the operating system knows it. \
                                 Type is what kind of interface GLPI recorded - Ethernet, \
                                 Wifi, Local for loopback. MAC is the hardware address, and \
                                 Addresses lists every IP on that interface, v4 and v6 \
                                 together.",
                            ),
                            InfoSection::new(
                                "This page could not be built on the current API",
                                "Network ports have a schema in GLPI's current API but no \
                                 address to fetch them from, and IP addresses are absent from \
                                 it entirely. The nearest it offers is a MAC per network \
                                 card, one request per machine, with no addresses attached. \
                                 The older API serves ports and addresses for the whole fleet \
                                 in two calls, which is why this page uses it.",
                            ),
                            InfoSection::new(
                                "Addresses are matched to the right interface",
                                "GLPI hangs an address off a name, and the name off a port, \
                                 so an address is traced back through both rather than merely \
                                 attributed to the machine. An address on the wrong interface \
                                 is usually the thing worth finding.",
                            ),
                        ],
                        info_links: vec![InfoLink::new("Computers", "front/computer.php")],
                        actions: rsx! {
                            RefreshControl {
                                interval_label: REFRESH_LABEL.to_string(),
                                on_refresh: move |_| rows.restart(),
                            }
                        },

                        if list.is_empty() {
                            div { class: "text-base-content/60",
                                "No computers in GLPI yet. That is different from an error: "
                                "nothing has been inventoried."
                            }
                        } else {
                            NetworkTable { rows: list.clone() }
                        }
                    }
                }
            },
            None => rsx! { Loading {} },
        }
    }
}

#[component]
fn NetworkTable(rows: Vec<NetworkRow>) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "table table-sm",
                thead {
                    tr {
                        th { "Computer" }
                        th { "Port" }
                        th { "Type" }
                        th { "MAC" }
                        th { "Addresses" }
                    }
                }
                tbody {
                    for r in rows.iter() {
                        // A machine with no port still gets a row. Dropping it
                        // would hide the one state this page flags, and the
                        // machine would simply be absent with nothing to say
                        // it had been looked at.
                        if r.empty {
                            tr { class: "bg-warning/5",
                                td { class: "font-medium",
                                    "{r.computer.clone().unwrap_or_else(|| \"(unnamed)\".into())}"
                                }
                                td { class: "text-warning", colspan: "4", "no interface reported" }
                            }
                        }
                        for (i, p) in r.ports.iter().enumerate() {
                            tr {
                                // The machine name on its first port only, so
                                // the eye groups the ports under it rather
                                // than reading the same name four times.
                                td { class: "font-medium",
                                    if i == 0 {
                                        "{r.computer.clone().unwrap_or_else(|| \"(unnamed)\".into())}"
                                    }
                                }
                                td { class: "font-mono text-xs",
                                    "{p.name.clone().unwrap_or_default()}"
                                }
                                td { class: "text-base-content/70",
                                    "{p.kind()}"
                                    if !p.is_dynamic {
                                        div { class: "text-xs text-base-content/50", "entered by hand" }
                                    }
                                }
                                td { class: "font-mono text-xs text-base-content/70",
                                    "{p.mac.clone().unwrap_or_default()}"
                                }
                                td {
                                    if p.ips.is_empty() {
                                        span { class: "text-base-content/40", "-" }
                                    } else {
                                        for ip in p.ips.iter() {
                                            div { class: "font-mono text-xs", "{ip}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The one that is not waiting on us.
#[component]
pub fn InvRefused() -> Element {
    let report = use_resource(|| async move { crate::api::fetch_refused().await });

    rsx! {
        InvNav {}
        Panel {
            title: "Refused".to_string(),
            subtitle: "Submissions GLPI threw away".to_string(),
            info: "Equipment GLPI rejected when an agent tried to submit it. The agent \
                   reports success and exits cleanly; nothing appears; nothing says why."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Why this is the failure worth catching",
                    "The agent runs, reports success and exits cleanly; GLPI rejects what it \
                     sent; the machine never appears, or its record silently stops being \
                     updated. Nothing in the agent's own log says so. Worse, the agent still \
                     counts as reporting on the Agents page, because GLPI records the moment \
                     of contact before it inspects what was sent - so this failure looks \
                     healthy from every other angle.",
                ),
                InfoSection::new(
                    "Where this comes from",
                    "GLPI's database, directly. There is no address for refused equipment \
                     anywhere in its API - not the current one, not the older one - so this \
                     is the single thing in the app that is not read through GLPI's own \
                     interface.",
                ),
                InfoSection::new(
                    "Does that step around GLPI's permissions?",
                    "It would, left alone, which is why it is not left alone. The rows shown \
                     are restricted to the same entities the API reports as visible, so this \
                     page means the same thing by \"everything\" as every other page. The \
                     account used holds read access to two tables and nothing else - less \
                     than the account the rest of the app already signs in with.",
                ),
            ],
            info_links: vec![
                InfoLink::new("Import rules", "front/ruleimportasset.php"),
                InfoLink::new("Agents", "front/agent.php"),
            ],

            match &*report.read_unchecked() {
                Some(Err(e)) => rsx! {
                    div { class: "alert alert-error",
                        div {
                            div { class: "font-semibold", "Could not read refusals." }
                            pre { class: "mt-1 text-xs break-all whitespace-pre-wrap", "{e}" }
                        }
                    }
                },

                // Not configured is not the same as nothing to report, and
                // must never render as the reassuring one.
                Some(Ok(r)) if !r.configured => rsx! {
                    div { class: "alert alert-warning",
                        div {
                            div { class: "font-semibold", "No database access configured." }
                            div { class: "text-sm",
                                "Refused submissions exist only in GLPI's database, so this \
                                page cannot say whether there are any. That is different \
                                from saying there are none."
                            }
                        }
                    }
                },

                Some(Ok(r)) if r.rows.is_empty() => rsx! {
                    div { class: "rounded-box border border-base-300 p-4 text-base-content/70",
                        "No refused submissions recorded. Every inventory GLPI received was "
                        "accepted."
                    }
                },

                Some(Ok(r)) => rsx! { RefusedTable { rows: r.rows.clone() } },

                None => rsx! {
                    div { class: "flex items-center gap-2 text-base-content/60",
                        span { class: "loading loading-spinner loading-sm" }
                        "Loading…"
                    }
                },
            }
        }
    }
}

#[component]
fn RefusedTable(rows: Vec<crate::api::Refused>) -> Element {
    rsx! {
        div { class: "overflow-x-auto",
            table { class: "table table-sm",
                thead {
                    tr {
                        th { "When" }
                        th { "Name" }
                        th { "Type" }
                        th { "Serial" }
                        th { "From" }
                        th { "Rule" }
                    }
                }
                tbody {
                    for r in rows.iter() {
                        tr {
                            td { class: "whitespace-nowrap text-base-content/80",
                                "{r.date_creation.clone().unwrap_or_default()}"
                            }
                            td { class: "font-medium", "{r.name.clone().unwrap_or_default()}" }
                            td { class: "text-base-content/80", "{r.itemtype.clone().unwrap_or_default()}" }
                            td { class: "font-mono text-xs", "{r.serial.clone().unwrap_or_default()}" }
                            td { class: "font-mono text-xs text-base-content/70", "{r.ip.clone().unwrap_or_default()}" }
                            td {
                                // GLPI replays the rules against this exact
                                // submission, which answers "why" far better
                                // than a rule name would.
                                if let Some(id) = r.id.into() {
                                    RuleLink { refused_id: id, rules_id: r.rules_id }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn RuleLink(refused_id: i64, rules_id: Option<i64>) -> Element {
    let href = crate::glpi_link::link(&format!(
        "front/rulesengine.test.php?sub_type=RuleImportAsset&refusedequipments_id={refused_id}"
    ));
    let label = match rules_id {
        Some(id) => format!("rule {id}"),
        None => "replay".to_string(),
    };

    rsx! {
        if let Some(h) = href {
            a {
                class: "link link-primary text-xs",
                href: "{h}",
                target: "_blank",
                rel: "noopener noreferrer",
                "{label} ↗"
            }
        } else {
            span { class: "text-xs text-base-content/60", "{label}" }
        }
    }
}
