// erga app icon — the mint heptagon crystal (soft3's sigil, the app's one
// button) as a shield on a near-black squircle, an angular Σ — the ergo
// mark — held inside. erga, the heroine for ergo.
//
//   swift packaging/icon.swift /tmp/erga-icon-1024.png
//
// Pure CoreGraphics: no dependencies, deterministic output.

import CoreGraphics
import Foundation
import ImageIO

let S: CGFloat = 1024
let out = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "/tmp/erga-icon-1024.png"

let space = CGColorSpace(name: CGColorSpace.sRGB)!
let ctx = CGContext(
    data: nil, width: Int(S), height: Int(S), bitsPerComponent: 8, bytesPerRow: 0,
    space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
)!

func mint(_ a: CGFloat) -> CGColor { CGColor(srgbRed: 125 / 255, green: 1.0, blue: 196 / 255, alpha: a) }
let bg = CGColor(srgbRed: 3 / 255, green: 6 / 255, blue: 5 / 255, alpha: 1)

// ── squircle plate (macOS icon grid: ~824pt content, big corner radius) ──
let m: CGFloat = 100
let plate = CGPath(
    roundedRect: CGRect(x: m, y: m, width: S - 2 * m, height: S - 2 * m),
    cornerWidth: 185, cornerHeight: 185, transform: nil
)
ctx.addPath(plate)
ctx.setFillColor(bg)
ctx.fillPath()

// faint mint aura rising from the center
ctx.saveGState()
ctx.addPath(plate)
ctx.clip()
let aura = CGGradient(
    colorsSpace: space,
    colors: [mint(0.11), mint(0.0)] as CFArray,
    locations: [0, 1]
)!
ctx.drawRadialGradient(
    aura,
    startCenter: CGPoint(x: 512, y: 512), startRadius: 0,
    endCenter: CGPoint(x: 512, y: 512), endRadius: 430, options: []
)

// ── the crystal ──────────────────────────────────────────────────────────
// The exported PNG flips CG's bottom-left origin, so -π/2 here lands the
// heptagon's vertex at the visual top — the shield points up.
func hept(_ r: CGFloat, _ phase: CGFloat) -> CGPath {
    let p = CGMutablePath()
    for i in 0..<7 {
        let a = phase - .pi / 2 + CGFloat(i) * 2 * .pi / 7
        let pt = CGPoint(x: 512 + r * cos(a), y: 512 + r * sin(a))
        if i == 0 { p.move(to: pt) } else { p.addLine(to: pt) }
    }
    p.closeSubpath()
    return p
}

// expanding glow rings, alpha falling — the app's idle halo
for (k, a) in [(1.09, 0.16), (1.18, 0.08), (1.28, 0.04)] {
    ctx.addPath(hept(300 * CGFloat(k), 0))
    ctx.setStrokeColor(mint(CGFloat(a)))
    ctx.setLineWidth(7)
    ctx.setLineJoin(.round)
    ctx.strokePath()
}

// the filled crystal, blooming
ctx.setShadow(offset: .zero, blur: 70, color: mint(0.85))
ctx.addPath(hept(300, 0))
ctx.setFillColor(mint(1.0))
ctx.fillPath()
ctx.setShadow(offset: .zero, blur: 0, color: nil)

// inner counter-rotated facet, cut in the plate colour
ctx.addPath(hept(196, -0.32))
ctx.setStrokeColor(CGColor(srgbRed: 3 / 255, green: 6 / 255, blue: 5 / 255, alpha: 0.5))
ctx.setLineWidth(6)
ctx.setLineJoin(.round)
ctx.strokePath()

// ── the Σ — ergo's mark, one heroic stroke ───────────────────────────────
let sigma = CGMutablePath()
sigma.move(to: CGPoint(x: 616, y: 648))
sigma.addLine(to: CGPoint(x: 428, y: 648))
sigma.addLine(to: CGPoint(x: 556, y: 512))
sigma.addLine(to: CGPoint(x: 428, y: 376))
sigma.addLine(to: CGPoint(x: 616, y: 376))
ctx.addPath(sigma)
ctx.setStrokeColor(bg)
ctx.setLineWidth(38)
ctx.setLineCap(.round)
ctx.setLineJoin(.round)
ctx.strokePath()

ctx.restoreGState()

// ── write the PNG ────────────────────────────────────────────────────────
let img = ctx.makeImage()!
let url = URL(fileURLWithPath: out) as CFURL
let dest = CGImageDestinationCreateWithURL(url, "public.png" as CFString, 1, nil)!
CGImageDestinationAddImage(dest, img, nil)
CGImageDestinationFinalize(dest)
print("wrote \(out)")
