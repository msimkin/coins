// Renders the tool's own coloured output to a PNG for the README.
//
// A markdown code block strips ANSI, so the screenshots there would be grey text.
// This reads real terminal output — escapes and all — and draws it with a
// monospaced font, so what the README shows is what the terminal shows.
//
//     CLICOLOR_FORCE=1 COLORTERM=truecolor COINS_WIDTH=78 coins > /tmp/out.txt
//     swift assets/screenshot.swift /tmp/out.txt docs/market.png
//
// Only the escapes theme.rs emits are understood: truecolor foregrounds, bold, and
// the resets for each.
import AppKit

let inPath  = CommandLine.arguments[1]
let outPath = CommandLine.arguments[2]
// The dark surface the palette was validated against, and its default ink.
let ground = NSColor(srgbRed: 0.102, green: 0.102, blue: 0.098, alpha: 1)
let ink    = NSColor(srgbRed: 0.898, green: 0.894, blue: 0.867, alpha: 1)

struct Run { var text: String; var color: NSColor; var bold: Bool }

func parse(_ line: String) -> [Run] {
    var runs: [Run] = []
    var text = "", color = ink, bold = false
    var i = line.startIndex
    func flush() {
        if !text.isEmpty { runs.append(Run(text: text, color: color, bold: bold)); text = "" }
    }
    while i < line.endIndex {
        if line[i] == "\u{1b}", line.index(after: i) < line.endIndex,
           line[line.index(after: i)] == "[" {
            var j = line.index(i, offsetBy: 2)
            var body = ""
            while j < line.endIndex, line[j] != "m" { body.append(line[j]); j = line.index(after: j) }
            flush()
            let parts = body.split(separator: ";").map { Int($0) ?? 0 }
            switch parts.first ?? 0 {
            case 0:  color = ink; bold = false
            case 1:  bold = true
            case 22: bold = false
            case 39: color = ink
            case 38 where parts.count >= 5 && parts[1] == 2:
                color = NSColor(srgbRed: CGFloat(parts[2])/255, green: CGFloat(parts[3])/255,
                                blue: CGFloat(parts[4])/255, alpha: 1)
            default: break
            }
            i = j < line.endIndex ? line.index(after: j) : j
        } else {
            text.append(line[i]); i = line.index(after: i)
        }
    }
    flush()
    return runs
}

/// The (column, row) of each inked dot in a braille cell, or nil if the character
/// is not one. Bit order is Unicode's: dots 1-3 down the left, 4-6 down the right,
/// then 7 and 8 as the bottom row.
func brailleDots(_ ch: Character) -> [(Int, Int)]? {
    guard let v = ch.unicodeScalars.first?.value, (0x2800...0x28FF).contains(v) else { return nil }
    let bits = Int(v - 0x2800)
    let map: [(Int, Int)] = [(0,0), (0,1), (0,2), (1,0), (1,1), (1,2), (0,3), (1,3)]
    return map.enumerated().filter { bits & (1 << $0.offset) != 0 }.map { $0.element }
}

let raw = try String(contentsOfFile: inPath, encoding: .utf8)
var lines = raw.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
// The tool ends with a blank line, which would become empty pixels at the bottom.
while let last = lines.last, last.trimmingCharacters(in: .whitespaces).isEmpty { lines.removeLast() }
let parsed = lines.map(parse)

// Menlo has the braille and box-drawing glyphs the charts are built from, and its
// box verticals meet across an integral line height — which is what makes the y
// axis read as one line rather than a column of ticks.
let scale: CGFloat = 2
let fontSize: CGFloat = 13 * scale
let font = NSFont(name: "Menlo", size: fontSize) ?? .monospacedSystemFont(ofSize: fontSize, weight: .regular)
let boldFont = NSFont(name: "Menlo-Bold", size: fontSize) ?? font
let cellW = ("M" as NSString).size(withAttributes: [.font: font]).width
let lineH = (font.ascender - font.descender + font.leading).rounded()
let pad = 18 * scale

let cols = parsed.map { $0.reduce(0) { $0 + $1.text.count } }.max() ?? 0
let W = Int((CGFloat(cols) * cellW + 2*pad).rounded())
let H = Int((CGFloat(lines.count) * lineH + 2*pad).rounded())

let ctx = CGContext(data: nil, width: W, height: H, bitsPerComponent: 8, bytesPerRow: 0,
                    space: CGColorSpace(name: CGColorSpace.sRGB)!,
                    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
ctx.setAllowsAntialiasing(true)
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: false)

// The window: the terminal's ground, with corners rounded enough to read as one.
let r = 12 * scale
let round = NSBezierPath(roundedRect: NSRect(x: 0, y: 0, width: CGFloat(W), height: CGFloat(H)),
                         xRadius: r, yRadius: r)
ground.setFill(); round.fill()

for (n, runs) in parsed.enumerated() {
    // Each glyph is placed in its own cell: the columns are what the layout means,
    // and letting the text engine kern them would bend the grid.
    var col = 0
    let y = CGFloat(H) - pad - CGFloat(n + 1) * lineH
    for run in runs {
        for ch in run.text {
            let x = pad + CGFloat(col) * cellW
            if let dots = brailleDots(ch) {
                // Drawn rather than typeset: no monospaced font carries the braille
                // block, so the system falls back to a braille *reading* face whose
                // dots are round and far apart, and the curve comes out as specks.
                // The tool's plot is a 2x4 dot grid, so the grid is what gets drawn.
                run.color.setFill()
                let dw = cellW / 2, dh = lineH / 4
                let size = min(dw, dh) * 0.95
                for (dx, dy) in dots {
                    let cx = x + (CGFloat(dx) + 0.5) * dw
                    let cy = y + lineH - (CGFloat(dy) + 0.5) * dh - font.descender * 0.5
                    NSBezierPath(roundedRect: NSRect(x: cx - size/2, y: cy - size/2,
                                                     width: size, height: size),
                                 xRadius: size * 0.35, yRadius: size * 0.35).fill()
                }
            } else {
                (String(ch) as NSString).draw(
                    at: NSPoint(x: x, y: y),
                    withAttributes: [.font: run.bold ? boldFont : font,
                                     .foregroundColor: run.color])
            }
            col += 1
        }
    }
}
NSGraphicsContext.restoreGraphicsState()

let image = ctx.makeImage()!
let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: outPath) as CFURL,
                                           "public.png" as CFString, 1, nil)!
CGImageDestinationAddImage(dest, image, nil)
guard CGImageDestinationFinalize(dest) else {
    FileHandle.standardError.write("could not write \(outPath)\n".data(using: .utf8)!)
    exit(1)
}
print("wrote \(outPath) at \(W)x\(H)")
