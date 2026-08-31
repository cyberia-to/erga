# bundle.nu — build erga.app and erga.dmg from the release binary.
#
#   nu packaging/bundle.nu
#
# Produces packaging/dist/erga.app and packaging/dist/erga-<ver>-arm64.dmg.
# Unsigned: first launch needs `xattr -dr com.apple.quarantine erga.app`
# (documented in the release notes) until we notarize with a Developer ID.

def main [] {
    let root = ($env.FILE_PWD | path dirname)
    let ver = "0.20.0"
    let dist = ($env.FILE_PWD | path join "dist")
    let app = ($dist | path join "erga.app")

    print "building release binary…"
    with-env {RUSTC_BOOTSTRAP: "1"} {
        cd $root
        # One binary: the window spawns *itself* as the miner, so there is no
        # second file to ship or to drift out of version with the first.
        ^cargo build --release -p erga-cli
    }
    let bin = ($root | path join "target/release/erga")
    if not ($bin | path exists) {
        error make {msg: $"binary not found at ($bin)"}
    }

    print "assembling erga.app…"
    rm -rf $app
    mkdir ($app | path join "Contents/MacOS")
    mkdir ($app | path join "Contents/Resources")
    cp ($env.FILE_PWD | path join "Info.plist") ($app | path join "Contents/Info.plist")
    cp $bin ($app | path join "Contents/MacOS/erga")
    cp ($env.FILE_PWD | path join "erga.icns") ($app | path join "Contents/Resources/erga.icns")
    "APPL????" | save -f ($app | path join "Contents/PkgInfo")

    # ad-hoc sign so Gatekeeper shows a name, not a corrupt binary
    ^codesign --force --deep --sign - $app

    print "building dmg…"
    let dmg = ($dist | path join $"erga-($ver)-arm64.dmg")
    rm -f $dmg
    let staging = ($dist | path join "dmg-staging")
    rm -rf $staging
    mkdir $staging
    cp -r $app ($staging | path join "erga.app")
    ^ln -s /Applications ($staging | path join "Applications")
    ^hdiutil create -volname $"erga ($ver)" -srcfolder $staging -ov -format UDZO $dmg
    rm -rf $staging

    let size = (ls $dmg | get size.0)
    print $"done: ($dmg) \(($size)\)"
}
