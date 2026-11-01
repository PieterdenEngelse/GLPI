<?php
/**
 * Dump GLPI's right -> (interface, tab, group) taxonomy as JSON, on stdout.
 *
 * WHY THIS EXISTS. The structure of GLPI's profile form - which tab and which
 * block each right appears under - is the only sensible way to present ~101
 * rights, and it is available through NO api: not v2.3, not v1, not
 * /api.php/doc.json (967 x-itemtype, zero rightname), and no glpi-console
 * command touches rights. It lives only in Profile::getRightsForForm().
 *
 * WHY IT CALLS GLPI RATHER THAN PARSING IT. Parsing src/Profile.php with a
 * regex looks cheaper and does not work. Of its 104 class-based entries, seven
 * cannot be resolved statically: one is `self::class`, and Certificate, Grid,
 * Item_DeviceSimcard, TicketTask and TicketTemplate declare no literal
 * $rightname at all - theirs is computed or inherited at runtime. Asking GLPI
 * is the only complete answer.
 *
 * WHY IT NEEDS THE glpi USER. getRightsForForm reaches CommonDBTM::getTable()
 * and so $DB->listFields(). A vendor/autoload.php-only bootstrap dies with
 * "Call to a member function listFields() on null", so a real database
 * connection is required, and the credentials live in config/ - readable only
 * by the glpi user.
 *
 * HOW IT BOOTSTRAPS, and one dead end. `inc/includes.php` looks like the
 * bootstrap and is not: in GLPI 11 it is a pure deprecation shim that warns
 * about removed globals and loads nothing, so requiring it gets you
 * `Class "Profile" not found`. The supported sequence is the one bin/console
 * uses - vendor/autoload.php, then boot the Kernel, which is what wires up the
 * database.
 *
 * Read-only. Nothing is written to GLPI. Run it via `just right-taxonomy`.
 */
$src = getenv('GLPI_SRC');
if ($src === false || $src === '') {
    fwrite(STDERR, "GLPI_SRC is not set\n");
    exit(1);
}

require $src . '/vendor/autoload.php';

// As bin/console does it. Booting the kernel is what establishes $DB; without
// it getRightsForForm dies inside $DB->listFields().
$kernel = new \Glpi\Kernel\Kernel();
$kernel->boot();

// Pinned, not the session's: the labels are only a comment here, and a dump
// that changes with the operator's locale would churn the committed file.
$_SESSION['glpilanguage'] = 'en_GB';

$all = \Profile::getRightsForForm('all');

$out = [];
foreach ($all as $interface => $tabs) {
    foreach ($tabs as $tab => $groups) {
        foreach ($groups as $group => $entries) {
            foreach ($entries as $e) {
                if (!is_array($e) || !isset($e['field'])) {
                    continue;
                }
                $out[] = [
                    'right'     => $e['field'],
                    'interface' => $interface,
                    'tab'       => $tab,
                    'group'     => $group,
                    // Carried for a human reading the file; the app shows
                    // GLPI's internal name, which is what you search for.
                    'label'     => is_string($e['label'] ?? null) ? $e['label'] : null,
                ];
            }
        }
    }
}

if (count($out) === 0) {
    fwrite(STDERR, "refusing to emit an empty taxonomy - getRightsForForm returned nothing\n");
    exit(1);
}

// Stable order, so re-running on an unchanged GLPI produces no diff.
usort($out, fn($a, $b) => [$a['right'], $a['interface'], $a['tab'], $a['group']]
                      <=> [$b['right'], $b['interface'], $b['tab'], $b['group']]);

echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE), "\n";
