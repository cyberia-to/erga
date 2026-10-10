def main [audit: path = '.'] {
    let audit = ($audit | path expand)
    let archive = ($audit | path join evidence.tar.gz)
    let expected = 'd8ac363e27d6fba2dc2f9a186420163a4b5533dac092cdeae63b0efc74a4999c'
    if (open --raw $archive | hash sha256) != $expected { error make {msg:'evidence archive differs'} }
    let created = (^mktemp -d /tmp/erga-rtable-audit.XXXXXX | complete)
    if $created.exit_code != 0 { error make {msg:$created.stderr} }
    let temporary = ($created.stdout | str trim)
    let result = try {
        let extraction = (^tar -xzf $archive -C $temporary | complete)
        if $extraction.exit_code != 0 { error make {msg:$extraction.stderr} }
        for pair in [[capsule-inventory.json inventory.json] [source-freeze.json source-freeze-v2.json] [gates.json gate-index.json]] {
            if (open --raw ($audit | path join $pair.0) | hash sha256) != (open --raw ($temporary | path join $pair.1) | hash sha256) {
                error make {msg: $"outer receipt differs: ($pair.0)"}
            }
        }
        let replay = (^nu ($temporary | path join verify.nu) $temporary | complete)
        if $replay.exit_code != 0 { error make {msg:$replay.stderr} }
        $replay.stdout | from json | insert archive_sha256 $expected
    } catch {|failure|
        rm -r $temporary
        error make {msg:$failure.msg}
    }
    rm -r $temporary
    $result | to json | print
}
