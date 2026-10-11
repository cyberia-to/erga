def main [audit: path = '.', --source: path] {
    let audit = ($audit | path expand)
    for row in (open --raw ($audit | path join SHA256SUMS) | lines | parse '{sha256}  {path}') {
        if (open --raw ($audit | path join $row.path) | hash sha256) != $row.sha256 {
            error make {msg: $"outer checksum differs: ($row.path)"}
        }
    }
    let archive = ($audit | path join evidence.tar.gz)
    let expected = 'b7df9f54716657fcfb82f57d05a87111ff1248df1e12fe30d82bf049eedfc072'
    if (open --raw $archive | hash sha256) != $expected { error make {msg: 'evidence archive differs'} }
    let made = (^mktemp -d /tmp/erga-mine-audit.XXXXXX | complete)
    if $made.exit_code != 0 { error make {msg: $made.stderr} }
    let temporary = ($made.stdout | str trim)
    let result = try {
        let extracted = (^tar -xzf $archive -C $temporary | complete)
        if $extracted.exit_code != 0 { error make {msg: $extracted.stderr} }
        for pair in [[capsule-inventory.json inventory.json] [source-freeze.json execution/current-source.json] [gates.json execution/gates.json]] {
            if (open --raw ($audit | path join $pair.0) | hash sha256) != (open --raw ($temporary | path join $pair.1) | hash sha256) {
                error make {msg: $"outer receipt differs: ($pair.0)"}
            }
        }
        let replay = (^nu ($temporary | path join verify.nu) $temporary | complete)
        if $replay.exit_code != 0 { error make {msg: $replay.stderr} }
        mut live_files = 0
        if $source != null {
            let checkout = ($source | path expand)
            let freeze = (open ($audit | path join source-freeze.json))
            for row in $freeze.files {
                if (open --raw ($checkout | path join $row.path) | hash sha256) != $row.sha256 {
                    error make {msg: $"live candidate differs: ($row.path)"}
                }
                $live_files += 1
            }
        }
        $replay.stdout | from json | insert archive_sha256 $expected | insert live_candidate_files $live_files
    } catch {|failure|
        rm -r $temporary
        error make {msg: $failure.msg}
    }
    rm -r $temporary
    $result | to json | print
}
