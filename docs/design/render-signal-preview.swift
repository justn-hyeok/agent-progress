import AppKit

// Terminal-cell layout study, rendered offscreen. No product-screen mutation.
let output = CommandLine.arguments[1]
let width = 1440, height = 210
let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height,
    bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
    colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
func color(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat) -> NSColor {
    NSColor(red: r / 255, green: g / 255, blue: b / 255, alpha: 1)
}
color(15, 18, 20).setFill()
NSRect(x: 0, y: 0, width: width, height: height).fill()
color(31, 39, 27).setFill()
NSRect(x: 0, y: 0, width: CGFloat(width) * 4 / 24, height: CGFloat(height)).fill()
let accent = color(199, 249, 108)
func text(_ value: String, _ x: CGFloat, _ y: CGFloat, _ ink: NSColor, bold: Bool = false) {
    let font = NSFont(name: bold ? "AppleSDGothicNeo-Bold" : "AppleSDGothicNeo-Regular", size: 19)!
    (value as NSString).draw(at: NSPoint(x: x, y: y), withAttributes: [.font: font, .foregroundColor: ink])
}
let main = color(242, 245, 238), secondary = color(155, 164, 155)
text("AGENT PROGRESS", 26, 165, accent, bold: true)
text("제품 완성", 245, 165, main, bold: true)
text("16%", 370, 165, accent, bold: true)
text("4 / 24 항목 완료", 435, 165, secondary)
text("현재 계획", 26, 129, secondary)
text("Codex 기준 구현과 실행 검증", 135, 129, main, bold: true)
text("Ⅱ  일시 중지", 26, 93, color(245, 194, 111), bold: true)
text("마지막 완료", 26, 57, secondary)
text("Codex 종단 흐름·현재 버전 연동 검증", 135, 57, main)
text("계획  docs/product-completion-plan.md   ·   목표  ap.project.json", 26, 21, color(114, 129, 116))
NSGraphicsContext.restoreGraphicsState()
try bitmap.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: output))
