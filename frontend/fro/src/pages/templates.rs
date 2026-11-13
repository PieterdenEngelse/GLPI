//! GLPI's four unrelated "template" mechanisms, one page each.
//!
//! One module rather than five files, following inv.rs: the pages share a
//! sub-header and nothing else would be factored out by splitting them.
//!
//! These exist because the word is overloaded to the point of being
//! misleading. A flagged row, a carrier of values, a form definition and a
//! message template have nothing in common but the name, and advice about one
//! is wrong about the other three. Screens links here rather than trying to
//! hold all four in a tab.
//!
//! Reference, not telemetry - nothing on these pages is live. Every claim was
//! read in the GLPI 12.0.1 source or its install schema and names where.
use crate::app::Route;
use crate::components::{InfoLink, InfoSection, Panel};
use dioxus::prelude::*;
use dioxus_router::{use_route, Link};

/// The sub-header. Four kinds, the active one lit.
#[component]
fn TemplatesNav() -> Element {
    let current = use_route::<Route>();
    let is = |r: &Route| std::mem::discriminant(&current) == std::mem::discriminant(r);

    let links = [
        (Route::TemplateRow {}, "flagged row"),
        (Route::TemplateValues {}, "values"),
        (Route::TemplateForm {}, "form"),
        (Route::TemplateMessage {}, "message"),
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

/// An inline link into GLPI.
#[component]
fn GlpiRef(label: String, path: String) -> Element {
    rsx! {
        if let Some(h) = crate::glpi_link::link(&path) {
            a {
                class: "link link-primary text-sm",
                href: "{h}",
                target: "_blank",
                rel: "noopener noreferrer",
                "{label} ↗"
            }
        }
    }
}

#[component]
pub fn TemplateRow() -> Element {
    rsx! {
        TemplatesNav {}
        Panel {
            title: "A flagged row".to_string(),
            info: "Read in install/mysql/glpi-12.0.0-empty.sql and CommonDBTM. The \
                   columns are \
                   is_template tinyint NOT NULL DEFAULT '0' and template_name \
                   varchar(255), indexed as is_template and as active_assets on \
                   (is_deleted, is_template) - the pair the list query adds, at \
                   SQLProvider lines 4947 and 4965 - and the value itself is \
                   unconstrained; maybeTemplate at line 3427 only reports whether the \
                   field exists, so the feature switches itself on for any type \
                   whose table has the column, and the meaning of 1 is the code's \
                   rather than the schema's - isTemplate at 3442 casts the column \
                   to a boolean while the template list at 5434 filters for 1 \
                   exactly. withtemplate is documented at lines 538, 2820 and \
                   2861 - 1 means a new TEMPLATE, 2 a new OBJECT FROM one, and \
                   mixing them up gives a template where a record was wanted. Not \
                   verified: which columns GLPI resets when instantiating, so \
                   check a template's output before trusting it with a field that \
                   must be unique."
                .to_string(),
            // Computers because they are what this app is about; the page is
            // per-itemtype, so there is no one link that covers all twenty.
            info_links: vec![InfoLink::new(
                "Computer templates",
                "front/setup.templates.php?itemtype=Computer&add=0",
            )],
            p { class: "mb-3 text-sm",
                "A row in the item's own table with "
                code { class: "text-xs", "is_template = 1" }
                ". Item here is any type whose table carries an is_template column, which is \
                 why four of the twenty below are not assets at all. It pre-fills the next \
                 record and has no say in what the form looks like."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Twenty tables carry it. " }
                    "Computers, monitors, printers, phones, peripherals, network equipment, racks, \
                     enclosures, PDUs, passive DC equipment, cables, certificates, domains, \
                     software, licences - and things that are not assets at all: budgets, \
                     contracts, projects and project tasks. Custom assets are one more."
                }
                li {
                    span { class: "font-medium", "It has its own name. " }
                    code { class: "text-xs", "template_name" }
                    " is separate from the item's name, so a template called \"Pool car\" can leave \
                     the actual name blank for whoever uses it."
                }
                li {
                    span { class: "font-medium", "Kept out of the item lists. " }
                    "An index is a sorted copy of the column with a pointer back to each row, \
                     so asking for one value means seeking the run of matching entries instead \
                     of reading the table. There are two per table, and they answer the two \
                     questions. "
                    code { class: "text-xs", "is_template" }
                    " alone serves the template picker, which asks for "
                    code { class: "text-xs", "is_template = 1" }
                    " - the short run, a handful per type. "
                    code { class: "text-xs", "active_assets" }
                    ", on "
                    code { class: "text-xs", "(is_deleted, is_template)" }
                    ", is the pair the item list filters on, in that order: the deleted flag \
                     first - 0 for the normal list, 1 for the bin - then is_template = 0 \
                     always. What that second one narrows is another matter, since nearly \
                     every row holds 0 for both, and whether the database uses the index or \
                     reads the table is its own decision - not something this app can check, \
                     as its database account reaches two tables and neither is this one."
                }
                li {
                    span { class: "font-medium", "Nothing to enable. " }
                    "The flag is a column, so it is there from the moment the type exists. On \
                     custom assets no capacity governs it."
                }
                li {
                    span { class: "font-medium", "The column is in the schema, the 1 is not. " }
                    "The table declares "
                    code { class: "text-xs", "is_template tinyint NOT NULL DEFAULT '0'" }
                    " and two indexes, and constrains the value no further - what 1 means is \
                     GLPI's own convention, written in two places that do not quite agree. \
                     isTemplate casts the column to a boolean, so any non-zero value counts as \
                     a template; the list you pick a template from queries "
                    code { class: "text-xs", "is_template = 1" }
                    " exactly. A row holding 2 would call itself a template and never be \
                     offered in that list. GLPI never writes one - but the column would take \
                     it, which is worth knowing before reading the flag from outside GLPI."
                }
                li {
                    span {
                        class: "font-medium text-red-orange",
                        "Custom fields come along, on custom assets only. "
                    }
                    "They live in one "
                    code { class: "text-xs", "custom_fields" }
                    " JSON column on the same row, so whatever copies the row copies them. That \
                     column is on "
                    code { class: "text-xs", "glpi_assets_assets" }
                    " and nowhere else - it occurs once in the whole schema - so for the other \
                     nineteen tables there is nothing to carry. A plugin can add fields to them, \
                     which is outside GLPI itself and outside what this page covers. Note what \
                     the row implies: every custom asset type shares that one table, separated \
                     by assets_assetdefinitions_id rather than by having a table of its own."
                }
                li {
                    span { class: "font-medium", "Making one. " }
                    "Open the type's own list in GLPI and click Templates in the header, the \
                     link beside Add; its tooltip reads Manage templates. Which menu holds \
                     that list follows the type - Assets for a computer, Management for a \
                     contract or a licence, Tools for a project - because the page takes its \
                     sector from the itemtype rather than being a setup screen of its own. \
                     New template there opens the type's ordinary form: fill in what is the \
                     same every time, give it a template name, and save. The URLs behind \
                     that, if you ever need them: Templates and Add are both "
                    code { class: "text-xs", "setup.templates.php" }
                    ", with "
                    code { class: "text-xs", "add=0" }
                    " and "
                    code { class: "text-xs", "add=1" }
                    " - because the table has the column, a blank form is no longer what Add \
                     opens. New template is that form at "
                    code { class: "text-xs", "id=-1&withtemplate=1" }
                    ", while picking a template from Add's list is "
                    code { class: "text-xs", "withtemplate=2" }
                    ", a real record pre-filled from it - and the link GLPI renders for a \
                     template already carries the parameter, because isTemplate appends it. \
                     Saving a template returns you to the template list, not the asset list."
                }
                li {
                    span { class: "font-medium", "Not inheritance. " }
                    "Editing the template leaves existing records alone, and deleting it does too - \
                     there is no link back."
                }
            }

            p { class: "mt-3 text-sm text-base-content/70",
                "No single page lists these: each type shows its own templates from its own menu \
                 entry, because a template is an instance of that type."
            }
        }
    }
}

#[component]
pub fn TemplateValues() -> Element {
    rsx! {
        TemplatesNav {}
        Panel {
            title: "A carrier of values".to_string(),
            subtitle: "A row whose columns mirror the item's fields".to_string(),
            info: "Six classes, all dropdowns, all pre-filling content rather than shaping a \
                   form. ProjectTaskTemplate extends CommonDropdown and its table carries \
                   projects_id, plan_start_date, plan_end_date, planned_duration, \
                   projectstates_id, projecttasktypes_id, percent_done and is_milestone - the \
                   task's own fields. AbstractITILChildTemplate, also a CommonDropdown, is \
                   the base for the four ITIL child templates."
                .to_string(),

            p { class: "mb-3 text-sm",
                "You pick one and it fills fields in. It creates nothing by itself and decides \
                 nothing about the form."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Project tasks and planning events. " }
                    "ProjectTaskTemplate and PlanningExternalEventTemplate. "
                    GlpiRef {
                        label: "Project task templates".to_string(),
                        path: "front/projecttasktemplate.php".to_string(),
                    }
                    " · "
                    GlpiRef {
                        label: "Planning event templates".to_string(),
                        path: "front/planningexternaleventtemplate.php".to_string(),
                    }
                }
                li {
                    span { class: "font-medium", "The four ITIL child templates. " }
                    "For a task, a solution, a followup and a validation - the things that hang off \
                     a ticket rather than the ticket itself. "
                    GlpiRef {
                        label: "Task templates".to_string(),
                        path: "front/tasktemplate.php".to_string(),
                    }
                    " · "
                    GlpiRef {
                        label: "Solution templates".to_string(),
                        path: "front/solutiontemplate.php".to_string(),
                    }
                }
                li {
                    span { class: "font-medium", "Project tasks have both kinds. " }
                    code { class: "text-xs", "glpi_projecttasks" }
                    " carries is_template AND there is a projecttasktemplates table, so that one \
                     type has a flagged task you copy and a dropdown you pick. Two places to look \
                     when one of them seems not to work."
                }
            }
        }
    }
}

#[component]
pub fn TemplateForm() -> Element {
    rsx! {
        TemplatesNav {}
        Panel {
            title: "A form definition".to_string(),
            subtitle: "The only template kind that decides what a screen looks like".to_string(),
            info: "ITILTemplate and its three subclasses, and why the table \
                   itself looks empty."
                .to_string(),
            info_sections: vec![
                InfoSection::new(
                    "Where this was read",
                    "glpi_tickettemplates holds id, name, entities_id, \
                     is_recursive, comment and allowed_statuses - no ticket fields \
                     at all. The behaviour lives in four tables per kind, and they \
                     are not alike: hiddenfields, readonlyfields and \
                     mandatoryfields carry only a template and a field number, \
                     unique on that pair, while predefinedfields alone adds a \
                     value column and has no such unique key. ITILTemplate extends \
                     CommonDropdown; TicketTemplate, ChangeTemplate and \
                     ProblemTemplate extend ITILTemplate.",
                ),
                InfoSection::new(
                    "Resolved from context",
                    "tickettemplates_id sits on glpi_entities, glpi_profiles and \
                     glpi_itilcategories, with separate _demand and _incident \
                     columns, so the DEFAULT template depends on the entity, the \
                     profile and the category you are in rather than being set in \
                     one place.",
                ),
            ],

            p { class: "mb-3 text-sm",
                "This is the kind that "
                span { class: "font-medium", "does" }
                " define a screen: which fields are mandatory, hidden, readonly or pre-filled."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Three of them. " }
                    "Tickets, changes and problems. "
                    GlpiRef {
                        label: "Ticket templates".to_string(),
                        path: "front/tickettemplate.php".to_string(),
                    }
                    " · "
                    GlpiRef {
                        label: "Change templates".to_string(),
                        path: "front/changetemplate.php".to_string(),
                    }
                    " · "
                    GlpiRef {
                        label: "Problem templates".to_string(),
                        path: "front/problemtemplate.php".to_string(),
                    }
                }
                li {
                    span { class: "font-medium", "The table is nearly empty on purpose. " }
                    "Four satellite tables carry the behaviour, one per kind of rule - for \
                     tickets "
                    code { class: "text-xs", "glpi_tickettemplatehiddenfields" }
                    ", "
                    code { class: "text-xs", "glpi_tickettemplatereadonlyfields" }
                    ", "
                    code { class: "text-xs", "glpi_tickettemplatemandatoryfields" }
                    " and "
                    code { class: "text-xs", "glpi_tickettemplatepredefinedfields" }
                    ", with the same four named for change and for problem, twelve in all. The \
                     first three hold nothing but a template and a field number, unique on that \
                     pair: the row existing is the rule. Only predefinedfields adds a value \
                     column, being the only one with something to say beyond yes. So a field \
                     can be mandatory in one template and hidden in another without either \
                     table growing a column."
                }
                li {
                    span { class: "font-medium", "Tickets have no is_template column. " }
                    "Nothing from the flagged-row page applies here, and nothing here applies \
                     there."
                }
                li {
                    span { class: "font-medium", "Templates are not the only way a screen gets \
                                                  decided. " }
                    "A custom asset definition does the same job without being a template: the \
                     field order is its own, and each field can be marked hidden or readonly \
                     per profile. It is reached from the definition rather than from a template \
                     list, which is why it sits on the "
                    Link { class: "link link-primary", to: Route::Screens { tab: String::new() }, "Screens page" }
                    " - but for a custom asset it is the thing to reach for, and no ITILTemplate \
                     covers it."
                }
            }
        }
    }
}

#[component]
pub fn TemplateMessage() -> Element {
    rsx! {
        TemplatesNav {}
        Panel {
            title: "A message".to_string(),
            subtitle: "Neither an instance, nor values, nor a form".to_string(),
            info: "NotificationTemplate, the fourth thing the word covers. \
                   glpi_notificationtemplates holds id, name, itemtype, comment and css - no \
                   message text; the subject and body live in \
                   glpi_notificationtemplatetranslations, one row per language. \
                   NotificationTemplate extends CommonDBTM, the only one of the ten template \
                   classes that is not a CommonDropdown descendant."
                .to_string(),

            p { class: "mb-3 text-sm",
                "An email body, versioned by language. It has nothing to do with records or forms \
                 and is here only because GLPI calls it a template too."
            }

            ul { class: "ml-5 list-disc space-y-2 text-sm",
                li {
                    span { class: "font-medium", "Bound to an itemtype. " }
                    "The itemtype column says what the notification is about, which is how GLPI \
                     offers the right tags when the body is written."
                }
                li {
                    span { class: "font-medium", "Text is per language. " }
                    "A translations table, so one template serves every locale rather than one \
                     template per language. "
                    GlpiRef {
                        label: "Notification templates".to_string(),
                        path: "front/notificationtemplate.php".to_string(),
                    }
                }
                li {
                    span { class: "font-medium", "The odd one out, structurally. " }
                    "Nine of the ten template classes descend from CommonDropdown. This one does \
                     not, which is a fair summary of how little it shares with the others."
                }
            }
        }
    }
}
