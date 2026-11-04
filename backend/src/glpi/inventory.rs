//! What the agents collected: hardware, software and network, fleet-wide.
//!
//! WHY ALL THREE RUN ON v1. The rule in CLAUDE.md is that v1 is for what v2.3
//! does not expose at all, and these three qualify for two separate reasons.
//! Verified by reading src/Glpi/Api/HL/Controller/ in GLPI 11.0.12 and by
//! probing the running install, not assumed:
//!
//!   * Every component, volume, OS and software install is reachable under
//!     v2.3 only beneath a single asset id - `/Assets/Computer/{id}/Volume`,
//!     `/Assets/{itemtype}/{id}/Component/{type}`. There is no collection
//!     route for any of them; `Volume` and `Assets/Computer/Volume` both
//!     answer 404. So a fleet-wide view costs one request per machine per
//!     kind, where v1 answers the same question for the whole estate in one.
//!   * NetworkPort is worse than expensive, it is absent: it has a v2.3
//!     schema but no route at any path, and IPAddress has zero occurrences
//!     anywhere under HL. A network page cannot be built on v2.3 at all.
//!
//! This is the same trade legacy.rs already documents for rights, and it is
//! NOT the MariaDB trade: every call here is still made as the service
//! account and still filtered by its profile and entity scope.
//!
//! WHAT IS JOINED TO WHAT. v1 serves GLPI's tables almost raw, so each of
//! these is a link table carrying `itemtype`/`items_id` plus a foreign key
//! into a catalogue of names:
//!
//!   Item_DeviceProcessor.deviceprocessors_id   -> DeviceProcessor.designation
//!   Item_DeviceMemory.devicememories_id        -> DeviceMemory.designation
//!   Item_Disk.filesystems_id                   -> Filesystem.name
//!   Item_OperatingSystem.operatingsystems_id   -> OperatingSystem.name
//!   Item_SoftwareVersion.softwareversions_id   -> SoftwareVersion.softwares_id
//!                                              -> Software.name
//!   IPAddress.items_id -> NetworkName.items_id -> NetworkPort.id
//!
//! Field names are GLPI's own throughout (`freesize`, `nbcores`,
//! `instantiation_type`), per the convention in CLAUDE.md, so this can be
//! diffed against the schema it reads.
use std::collections::{HashMap, HashSet};

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use super::legacy::{Legacy, Session};

/// An integer field, whether v1 sends it as a number or a numeric string.
///
/// Both shapes occur across these tables - v2.3 reports a processor's
/// frequency as the string "2100" where v1 sends 2100 - and a column that
/// changes type across a GLPI upgrade should cost a blank cell at worst, not
/// a parse failure for the whole fleet.
fn num(v: &Value, key: &str) -> Option<i64> {
    let f = v.get(key)?;
    f.as_i64().or_else(|| f.as_str()?.trim().parse().ok())
}

/// A string field, with empty treated as absent.
///
/// v1 returns "" rather than null for several of these (`arch` on
/// SoftwareVersion, `ifalias` on NetworkPort), and an empty string rendered
/// into a table reads as a value that happens to be blank rather than as
/// nothing recorded.
fn text(v: &Value, key: &str) -> Option<String> {
    let s = v.get(key)?.as_str()?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// A boolean field. v1 sends GLPI's tinyint columns as 0/1, not true/false.
fn flag(v: &Value, key: &str) -> bool {
    v.get(key)
        .map(|f| f.as_bool().unwrap_or_else(|| f.as_i64().unwrap_or(0) != 0))
        .unwrap_or(false)
}

/// Rows GLPI has soft-deleted. Still served by v1, and showing them would
/// report hardware a machine no longer has.
fn deleted(v: &Value) -> bool {
    flag(v, "is_deleted") || flag(v, "is_deleted_item")
}

/// id -> display name, from a v1 catalogue collection.
///
/// `key` differs by table: GLPI's component catalogues call it `designation`
/// and its dropdowns call it `name`.
fn catalogue(rows: &[Value], key: &str) -> HashMap<i64, String> {
    rows.iter()
        .filter_map(|r| Some((num(r, "id")?, text(r, key)?)))
        .collect()
}

/// The computer a link row belongs to, or None if it belongs to something
/// else.
///
/// The itemtype check is not decoration: GLPI attaches components, volumes
/// and ports to printers and network equipment too, and `items_id` is only
/// unique within a type - without this, a printer's id 1 would file its
/// network port under computer 1.
fn computer_of(v: &Value) -> Option<i64> {
    (v.get("itemtype").and_then(Value::as_str) == Some("Computer")).then(|| num(v, "items_id"))?
}

/// Group link rows by the computer they belong to, dropping deleted rows and
/// anything attached to another kind of asset.
fn by_computer(rows: Vec<Value>) -> HashMap<i64, Vec<Value>> {
    let mut out: HashMap<i64, Vec<Value>> = HashMap::new();
    for row in rows {
        if deleted(&row) {
            continue;
        }
        if let Some(id) = computer_of(&row) {
            out.entry(id).or_default().push(row);
        }
    }
    out
}

// ---------------------------------------------------------------- hardware

#[derive(Debug, Clone, Serialize)]
pub struct Processor {
    pub designation: Option<String>,
    pub nbcores: Option<i64>,
    pub nbthreads: Option<i64>,
    /// MHz, as GLPI stores it.
    pub frequency: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryModule {
    pub designation: Option<String>,
    /// MB, as GLPI stores it.
    pub size: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Disk {
    pub name: Option<String>,
    pub device: Option<String>,
    pub mountpoint: Option<String>,
    pub filesystem: Option<String>,
    /// MB, as GLPI stores it.
    pub totalsize: Option<i64>,
    pub freesize: Option<i64>,
    /// GLPI's encryption_status: 0 is "not encrypted". Reported as the raw
    /// number rather than a yes/no because the other values distinguish
    /// partially-encrypted states this app has no business collapsing.
    pub encryption_status: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperatingSystem {
    pub name: Option<String>,
    pub kernel_version: Option<String>,
    pub architecture: Option<String>,
}

/// One machine's hardware, as the agent last reported it.
#[derive(Debug, Clone, Serialize)]
pub struct HardwareRow {
    /// The computer this describes, named `items_id` because that is the
    /// column every link table joins on.
    pub items_id: i64,
    pub computer: Option<String>,

    pub processors: Vec<Processor>,
    pub memory: Vec<MemoryModule>,
    /// Installed memory in MB, summed across modules. Precomputed because the
    /// sum is the figure worth reading and a per-module list is the detail.
    pub memory_mb: i64,
    pub disks: Vec<Disk>,
    pub operatingsystem: Option<OperatingSystem>,

    /// Nothing at all was collected for this machine.
    ///
    /// Its own flag rather than left for the reader to infer from four empty
    /// lists: a computer with no hardware recorded is the interesting row on
    /// this page - it means an asset exists but no inventory ever landed
    /// against it - and that is not the same as a machine whose disks simply
    /// were not reported.
    pub empty: bool,
}

/// Every computer's hardware, in one pass over the fleet.
///
/// `computers` is the id -> name map from v2.3's `Assets/Computer`, and it
/// decides the scope: a machine absent from it is absent here. That keeps
/// this page meaning the same thing by "everything" as the Computers page,
/// which is the same reasoning refused.rs applies to entities.
pub async fn hardware(
    legacy: &Legacy,
    session: &Session,
    computers: &HashMap<i64, String>,
) -> Result<Vec<HardwareRow>> {
    // Concurrently: ten independent collections, and a page that waited for
    // each in turn would take ten round trips to GLPI rather than one.
    let (
        processors,
        cpu_names,
        memory,
        mem_names,
        disks,
        filesystems,
        os,
        os_names,
        kernels,
        arches,
    ) = tokio::try_join!(
        legacy.get_paged(session, "Item_DeviceProcessor"),
        legacy.get_paged(session, "DeviceProcessor"),
        legacy.get_paged(session, "Item_DeviceMemory"),
        legacy.get_paged(session, "DeviceMemory"),
        legacy.get_paged(session, "Item_Disk"),
        legacy.get_paged(session, "Filesystem"),
        legacy.get_paged(session, "Item_OperatingSystem"),
        legacy.get_paged(session, "OperatingSystem"),
        legacy.get_paged(session, "OperatingSystemKernelVersion"),
        legacy.get_paged(session, "OperatingSystemArchitecture"),
    )?;

    let cpu_names = catalogue(&cpu_names, "designation");
    let mem_names = catalogue(&mem_names, "designation");
    let filesystems = catalogue(&filesystems, "name");
    let os_names = catalogue(&os_names, "name");
    let kernels = catalogue(&kernels, "name");
    let arches = catalogue(&arches, "name");

    let mut processors = by_computer(processors);
    let mut memory = by_computer(memory);
    let mut disks = by_computer(disks);
    let mut os = by_computer(os);

    let mut out: Vec<HardwareRow> = computers
        .iter()
        .map(|(&id, name)| {
            let processors: Vec<Processor> = processors
                .remove(&id)
                .unwrap_or_default()
                .iter()
                .map(|r| Processor {
                    designation: num(r, "deviceprocessors_id")
                        .and_then(|k| cpu_names.get(&k).cloned()),
                    nbcores: num(r, "nbcores"),
                    nbthreads: num(r, "nbthreads"),
                    frequency: num(r, "frequency"),
                })
                .collect();

            let memory: Vec<MemoryModule> = memory
                .remove(&id)
                .unwrap_or_default()
                .iter()
                .map(|r| MemoryModule {
                    designation: num(r, "devicememories_id")
                        .and_then(|k| mem_names.get(&k).cloned()),
                    size: num(r, "size"),
                })
                .collect();

            let disks: Vec<Disk> = disks
                .remove(&id)
                .unwrap_or_default()
                .iter()
                .map(|r| Disk {
                    name: text(r, "name"),
                    device: text(r, "device"),
                    mountpoint: text(r, "mountpoint"),
                    filesystem: num(r, "filesystems_id").and_then(|k| filesystems.get(&k).cloned()),
                    totalsize: num(r, "totalsize"),
                    freesize: num(r, "freesize"),
                    encryption_status: num(r, "encryption_status"),
                })
                .collect();

            // One OS per machine. GLPI's table allows more than one row and
            // the first is taken rather than guessing which would win.
            let operatingsystem =
                os.remove(&id)
                    .unwrap_or_default()
                    .first()
                    .map(|r| OperatingSystem {
                        name: num(r, "operatingsystems_id").and_then(|k| os_names.get(&k).cloned()),
                        kernel_version: num(r, "operatingsystemkernelversions_id")
                            .and_then(|k| kernels.get(&k).cloned()),
                        architecture: num(r, "operatingsystemarchitectures_id")
                            .and_then(|k| arches.get(&k).cloned()),
                    });

            HardwareRow {
                items_id: id,
                computer: Some(name.clone()),
                memory_mb: memory.iter().filter_map(|m| m.size).sum(),
                empty: processors.is_empty()
                    && memory.is_empty()
                    && disks.is_empty()
                    && operatingsystem.is_none(),
                processors,
                memory,
                disks,
                operatingsystem,
            }
        })
        .collect();

    // Machines with nothing collected first - an asset no inventory ever
    // landed against is the reason to open this page - then by name so the
    // order is stable between refreshes.
    out.sort_by(|a, b| {
        b.empty
            .cmp(&a.empty)
            .then_with(|| a.computer.cmp(&b.computer))
    });
    Ok(out)
}

// ---------------------------------------------------------------- software

/// One software title and where it is installed.
#[derive(Debug, Clone, Serialize)]
pub struct SoftwareRow {
    pub softwares_id: i64,
    pub name: Option<String>,
    /// Distinct version strings installed across the fleet, sorted.
    ///
    /// More than one is worth seeing on its own: a title at three versions
    /// across an estate is the normal shape of a half-finished rollout.
    pub versions: Vec<String>,
    /// Installations counted across every machine.
    pub installs: i64,
    /// Machines carrying it. Differs from `installs` when one machine holds
    /// two versions of the same title.
    pub computers: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SoftwareReport {
    pub rows: Vec<SoftwareRow>,
    /// Machines with at least one install. Not the sum of the per-row counts
    /// - that would count a machine once per title it carries.
    pub computers_covered: i64,
    pub installs: i64,
}

/// Every software title installed across the fleet, most widely installed
/// first.
///
/// THE JOIN THIS EXISTS FOR. A v2.3 software install names only its version
/// ("1.33.7") and never its title, and v2.3 serves versions only under
/// `/Assets/Software/{id}/Version` - one request per title, about 1330 on
/// this install. v1 serves the whole `SoftwareVersion` table in one call, so
/// the map that makes the installs legible costs a single request.
pub async fn software(
    legacy: &Legacy,
    session: &Session,
    computers: &HashMap<i64, String>,
) -> Result<SoftwareReport> {
    let (installs, versions, titles) = tokio::try_join!(
        legacy.get_paged(session, "Item_SoftwareVersion"),
        legacy.get_paged(session, "SoftwareVersion"),
        legacy.get_paged(session, "Software"),
    )?;

    // version id -> (software id, version string)
    let versions: HashMap<i64, (i64, Option<String>)> = versions
        .iter()
        .filter_map(|v| Some((num(v, "id")?, (num(v, "softwares_id")?, text(v, "name")))))
        .collect();
    let titles = catalogue(&titles, "name");

    // Accumulated per title rather than per install: the page answers "what
    // is deployed and how widely", and one row per install would be 1468
    // rows for a single machine.
    struct Tally {
        versions: HashSet<String>,
        installs: i64,
        computers: HashSet<i64>,
    }
    let mut tally: HashMap<i64, Tally> = HashMap::new();
    let mut covered: HashSet<i64> = HashSet::new();

    for row in &installs {
        if deleted(row) || flag(row, "is_template_item") {
            continue;
        }
        // Scope, as in hardware(): a machine v2.3 does not show this account
        // must not appear here either.
        let Some(computer) = computer_of(row).filter(|c| computers.contains_key(c)) else {
            continue;
        };
        let Some((software_id, version)) =
            num(row, "softwareversions_id").and_then(|v| versions.get(&v))
        else {
            continue;
        };

        covered.insert(computer);
        let t = tally.entry(*software_id).or_insert_with(|| Tally {
            versions: HashSet::new(),
            installs: 0,
            computers: HashSet::new(),
        });
        t.installs += 1;
        t.computers.insert(computer);
        if let Some(v) = version {
            t.versions.insert(v.clone());
        }
    }

    let mut rows: Vec<SoftwareRow> = tally
        .into_iter()
        .map(|(softwares_id, t)| {
            let mut versions: Vec<String> = t.versions.into_iter().collect();
            versions.sort();
            SoftwareRow {
                softwares_id,
                name: titles.get(&softwares_id).cloned(),
                versions,
                installs: t.installs,
                computers: t.computers.len() as i64,
            }
        })
        .collect();

    // Most widely installed first, then by name so equal counts do not
    // reshuffle between refreshes.
    rows.sort_by(|a, b| {
        b.computers
            .cmp(&a.computers)
            .then_with(|| b.installs.cmp(&a.installs))
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(SoftwareReport {
        installs: rows.iter().map(|r| r.installs).sum(),
        computers_covered: covered.len() as i64,
        rows,
    })
}

// ----------------------------------------------------------------- network

#[derive(Debug, Clone, Serialize)]
pub struct Port {
    pub id: i64,
    pub name: Option<String>,
    pub mac: Option<String>,
    /// Which kind of port GLPI recorded - NetworkPortEthernet,
    /// NetworkPortWifi, NetworkPortLocal and so on.
    pub instantiation_type: Option<String>,
    pub logical_number: Option<i64>,
    /// Addresses on this port, v4 and v6 together, as GLPI holds them.
    pub ips: Vec<String>,
    /// Created by inventory rather than typed in by hand.
    pub is_dynamic: bool,
}

/// One machine's network ports.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkRow {
    pub items_id: i64,
    pub computer: Option<String>,
    pub ports: Vec<Port>,
    /// No port was recorded for this machine at all.
    pub empty: bool,
}

/// Every machine's network ports and addresses.
///
/// THIS PAGE CANNOT BE BUILT ON v2.3. NetworkPort has a v2.3 schema but no
/// route at any path - `Assets/NetworkPort` answers 404 and it is absent from
/// the `Assets` index - and IPAddress has zero occurrences anywhere under
/// src/Glpi/Api/HL/. The nearest v2.3 offers is a MAC per network card under
/// `/Assets/Computer/{id}/Component/NetworkCard`, which is one request per
/// machine and still has no addresses on it.
pub async fn network(
    legacy: &Legacy,
    session: &Session,
    computers: &HashMap<i64, String>,
) -> Result<Vec<NetworkRow>> {
    let (ports, names, ips) = tokio::try_join!(
        legacy.get_paged(session, "NetworkPort"),
        legacy.get_paged(session, "NetworkName"),
        legacy.get_paged(session, "IPAddress"),
    )?;

    // An address hangs off a NetworkName, which hangs off a NetworkPort, so
    // the port an IP belongs to takes two hops. IPAddress does carry
    // mainitems_id pointing straight at the computer, but that would only
    // say which machine holds the address and not which interface - and an
    // address on the wrong interface is the thing a reader comes here to
    // find.
    let name_to_port: HashMap<i64, i64> = names
        .iter()
        .filter(|n| !deleted(n))
        .filter_map(|n| {
            (n.get("itemtype").and_then(Value::as_str) == Some("NetworkPort"))
                .then(|| Some((num(n, "id")?, num(n, "items_id")?)))?
        })
        .collect();

    let mut port_ips: HashMap<i64, Vec<String>> = HashMap::new();
    for ip in &ips {
        if deleted(ip) {
            continue;
        }
        let Some(address) = text(ip, "name") else {
            continue;
        };
        if let Some(port) = num(ip, "items_id").and_then(|n| name_to_port.get(&n)) {
            port_ips.entry(*port).or_default().push(address);
        }
    }
    for list in port_ips.values_mut() {
        list.sort();
    }

    let mut ports = by_computer(ports);

    let mut out: Vec<NetworkRow> = computers
        .iter()
        .map(|(&id, name)| {
            let mut ports: Vec<Port> = ports
                .remove(&id)
                .unwrap_or_default()
                .iter()
                .filter_map(|p| {
                    let port_id = num(p, "id")?;
                    Some(Port {
                        id: port_id,
                        name: text(p, "name"),
                        mac: text(p, "mac"),
                        instantiation_type: text(p, "instantiation_type"),
                        logical_number: num(p, "logical_number"),
                        ips: port_ips.get(&port_id).cloned().unwrap_or_default(),
                        is_dynamic: flag(p, "is_dynamic"),
                    })
                })
                .collect();
            ports.sort_by(|a, b| {
                a.logical_number
                    .cmp(&b.logical_number)
                    .then_with(|| a.name.cmp(&b.name))
            });

            NetworkRow {
                items_id: id,
                computer: Some(name.clone()),
                empty: ports.is_empty(),
                ports,
            }
        })
        .collect();

    out.sort_by(|a, b| {
        b.empty
            .cmp(&a.empty)
            .then_with(|| a.computer.cmp(&b.computer))
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_integer_is_read_whether_v1_sends_a_number_or_a_string() {
        let v = json!({ "a": 2100, "b": "2100", "c": "", "d": null });
        assert_eq!(num(&v, "a"), Some(2100));
        assert_eq!(num(&v, "b"), Some(2100));
        assert_eq!(num(&v, "c"), None);
        assert_eq!(num(&v, "d"), None);
        assert_eq!(num(&v, "missing"), None);
    }

    /// v1 returns "" rather than null for several columns, and an empty
    /// string in a table cell reads as a recorded blank rather than as
    /// nothing recorded.
    #[test]
    fn an_empty_string_is_absent_not_a_value() {
        let v = json!({ "a": "wlp0s20f3", "b": "", "c": "  ", "d": null });
        assert_eq!(text(&v, "a").as_deref(), Some("wlp0s20f3"));
        assert_eq!(text(&v, "b"), None);
        assert_eq!(text(&v, "c"), None);
        assert_eq!(text(&v, "d"), None);
    }

    /// GLPI's tinyint columns arrive as 0/1, not as JSON booleans.
    #[test]
    fn booleans_arrive_as_integers() {
        let v = json!({ "is_dynamic": 1, "is_deleted": 0, "real": true });
        assert!(flag(&v, "is_dynamic"));
        assert!(!flag(&v, "is_deleted"));
        assert!(flag(&v, "real"));
        assert!(!flag(&v, "missing"));
    }

    /// The check that keeps a printer's components off a computer's row.
    /// `items_id` is unique only within an itemtype.
    #[test]
    fn only_rows_attached_to_a_computer_are_claimed() {
        assert_eq!(
            computer_of(&json!({ "itemtype": "Computer", "items_id": 1 })),
            Some(1)
        );
        assert_eq!(
            computer_of(&json!({ "itemtype": "Printer", "items_id": 1 })),
            None
        );
        assert_eq!(computer_of(&json!({ "items_id": 1 })), None);
    }

    #[test]
    fn soft_deleted_rows_are_dropped() {
        let rows = vec![
            json!({ "itemtype": "Computer", "items_id": 1, "is_deleted": 0 }),
            json!({ "itemtype": "Computer", "items_id": 1, "is_deleted": 1 }),
            // Deleted on the asset's side rather than the link's.
            json!({ "itemtype": "Computer", "items_id": 1, "is_deleted_item": 1 }),
        ];
        assert_eq!(by_computer(rows).get(&1).map(Vec::len), Some(1));
    }

    #[test]
    fn a_catalogue_indexes_by_id() {
        let rows = vec![
            json!({ "id": 1, "designation": "Intel(R) Core(TM) i3-10110U CPU @ 2.10GHz" }),
            json!({ "id": 2, "designation": "" }),
        ];
        let c = catalogue(&rows, "designation");
        assert_eq!(
            c.get(&1).map(String::as_str),
            Some("Intel(R) Core(TM) i3-10110U CPU @ 2.10GHz")
        );
        // An empty designation is not a name, so it is absent rather than
        // mapping the id to a blank.
        assert_eq!(c.get(&2), None);
    }
}
