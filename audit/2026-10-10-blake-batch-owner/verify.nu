def main [audit: path = '.'] {
    let audit = ($audit | path expand)
    let archive = ($audit | path join evidence.tar.gz)
    if (open --raw $archive | hash sha256) != '169ebb6357b95fd8c4252b6ff108a6d5224135bb50071a61ccaf08b3427bdb55' {
        error make {msg: 'evidence archive differs'}
    }
    let created = (^mktemp -d /tmp/erga-blake-audit.XXXXXX | complete)
    if $created.exit_code != 0 { error make {msg: $created.stderr} }
    let temporary = ($created.stdout | str trim)
    let result = try {
        let extracted = (^tar -xzf $archive -C $temporary | complete)
        if $extracted.exit_code != 0 { error make {msg: $extracted.stderr} }
        for pair in [[capsule-inventory.json inventory.json] [source-freeze.json source-freeze.json]] {
            if (open --raw ($audit | path join $pair.0) | hash sha256) != (open --raw ($temporary | path join $pair.1) | hash sha256) {
                error make {msg: $"outer receipt differs: ($pair.0)"}
            }
        }
        let replay = (^nu ($temporary | path join verify.nu) $temporary | complete)
        if $replay.exit_code != 0 { error make {msg: $replay.stderr} }
        $replay.stdout | from json | insert archive_sha256 '169ebb6357b95fd8c4252b6ff108a6d5224135bb50071a61ccaf08b3427bdb55'
    } catch {|failure|
        rm -r $temporary
        error make {msg: $failure.msg}
    }
    rm -r $temporary
    $result | to json | print
}
