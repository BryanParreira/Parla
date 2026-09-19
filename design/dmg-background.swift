// Renders the background of the installer DMG window at 1x and 2x.
//   swift design/dmg-background.swift design
// then `tiffutil -cathidpicheck` joins them so Finder picks the sharp one on Retina.
//
// Layout must match the icon positions in scripts/release.sh.

import AppKit

let width: CGFloat = 640
let height: CGFloat = 400
let appCenter = CGPoint(x: 170, y: 205)      // Finder coordinates, origin top-left
let applicationsCenter = CGPoint(x: 470, y: 205)

func render(scale: CGFloat, to path: String) {
  let pixelsWide = Int(width * scale)
  let pixelsHigh = Int(height * scale)
  let rep = NSBitmapImageRep(
    bitmapDataPlanes: nil, pixelsWide: pixelsWide, pixelsHigh: pixelsHigh, bitsPerSample: 8,
    samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
    bytesPerRow: 0, bitsPerPixel: 0)!
  rep.size = NSSize(width: width, height: height)

  NSGraphicsContext.saveGraphicsState()
  let context = NSGraphicsContext(bitmapImageRep: rep)!
  NSGraphicsContext.current = context
  let cg = context.cgContext
  // Flip so the drawing code uses Finder's top-left coordinates.
  cg.translateBy(x: 0, y: height)
  cg.scaleBy(x: 1, y: -1)

  drawBackdrop(cg)
  drawSphere(cg, center: CGPoint(x: width / 2, y: 205), radius: 150)
  drawArrow(cg, from: appCenter.x + 78, to: applicationsCenter.x - 78, y: appCenter.y)
  drawLabelPills(cg)
  drawText()

  NSGraphicsContext.restoreGraphicsState()
  let data = rep.representation(using: .png, properties: [:])!
  try! data.write(to: URL(fileURLWithPath: path))
}

func drawBackdrop(_ cg: CGContext) {
  let space = CGColorSpaceCreateDeviceRGB()
  let base = CGGradient(
    colorsSpace: space,
    colors: [
      CGColor(red: 0.078, green: 0.078, blue: 0.086, alpha: 1),
      CGColor(red: 0.039, green: 0.039, blue: 0.043, alpha: 1),
    ] as CFArray, locations: [0, 1])!
  cg.drawLinearGradient(base, start: .zero, end: CGPoint(x: 0, y: height), options: [])

  let glow = CGGradient(
    colorsSpace: space,
    colors: [CGColor(gray: 1, alpha: 0.07), CGColor(gray: 1, alpha: 0)] as CFArray,
    locations: [0, 1])!
  cg.drawRadialGradient(
    glow, startCenter: CGPoint(x: width / 2, y: 205), startRadius: 0,
    endCenter: CGPoint(x: width / 2, y: 205), endRadius: 260, options: [])
}

// The same dotted sphere as the recording orb, large and faint, so the installer
// feels like the app before it is even opened.
func drawSphere(_ cg: CGContext, center: CGPoint, radius: CGFloat) {
  let yaw = 0.6
  let tilt = 0.38
  let rings = 18
  let density = 56.0
  var dots: [(x: CGFloat, y: CGFloat, z: Double)] = []
  for ring in 0...rings {
    let lat = -Double.pi / 2 + Double(ring) / Double(rings) * Double.pi
    let wave = 0.62 * sin(1.3 - Double(ring) * 0.52) + 0.38 * sin(0.9 + Double(ring) * 0.83)
    let r = 0.9 + 0.05 * wave
    let count = max(1, Int((abs(cos(lat)) * density).rounded()))
    for j in 0..<count {
      let lon = Double(j) / Double(count) * 2 * Double.pi
      let x = cos(lat) * cos(lon) * r
      let y = sin(lat) * r
      let z = cos(lat) * sin(lon) * r
      let x1 = x * cos(yaw) + z * sin(yaw)
      let z1 = -x * sin(yaw) + z * cos(yaw)
      let y1 = y * cos(tilt) - z1 * sin(tilt)
      let z2 = y * sin(tilt) + z1 * cos(tilt)
      dots.append((center.x + CGFloat(x1) * radius, center.y - CGFloat(y1) * radius, z2))
    }
  }
  for dot in dots.sorted(by: { $0.z < $1.z }) {
    let depth = (dot.z + 1) / 2
    let size = CGFloat(0.7 + 1.5 * depth)
    cg.setFillColor(CGColor(gray: 1, alpha: CGFloat(0.03 + 0.11 * depth)))
    cg.fillEllipse(in: CGRect(x: dot.x - size, y: dot.y - size, width: size * 2, height: size * 2))
  }
}

// A trail of dots that grows toward Applications, ending in a dotted chevron.
func drawArrow(_ cg: CGContext, from start: CGFloat, to end: CGFloat, y: CGFloat) {
  let steps = 11
  for i in 0..<steps {
    let t = CGFloat(i) / CGFloat(steps - 1)
    let x = start + (end - 18 - start) * t
    let r = 1.3 + 1.2 * t
    cg.setFillColor(CGColor(gray: 1, alpha: 0.25 + 0.6 * t))
    cg.fillEllipse(in: CGRect(x: x - r, y: y - r, width: r * 2, height: r * 2))
  }
  let tip = CGPoint(x: end, y: y)
  for i in 1...3 {
    let offset = CGFloat(i) * 6
    for sign in [-1.0, 1.0] as [CGFloat] {
      let r: CGFloat = 2.4
      let point = CGPoint(x: tip.x - offset, y: tip.y + sign * offset)
      cg.setFillColor(CGColor(gray: 1, alpha: 0.85))
      cg.fillEllipse(in: CGRect(x: point.x - r, y: point.y - r, width: r * 2, height: r * 2))
    }
  }
  cg.setFillColor(CGColor(gray: 1, alpha: 0.95))
  cg.fillEllipse(in: CGRect(x: tip.x - 2.6, y: tip.y - 2.6, width: 5.2, height: 5.2))
}

// Finder draws icon labels black in light mode and white in dark mode, and neither can
// be changed. A mid-grey frosted pill behind each label keeps both readable.
func drawLabelPills(_ cg: CGContext) {
  let labelY: CGFloat = 280
  for center in [appCenter, applicationsCenter] {
    let rect = CGRect(x: center.x - 54, y: labelY - 11.5, width: 108, height: 23)
    cg.addPath(CGPath(roundedRect: rect, cornerWidth: 11.5, cornerHeight: 11.5, transform: nil))
    cg.setFillColor(CGColor(gray: 1, alpha: 0.46))
    cg.fillPath()
  }
}

func drawText() {
  func draw(_ string: String, size: CGFloat, weight: NSFont.Weight, alpha: CGFloat, y: CGFloat, tracking: CGFloat = 0) {
    let style = NSMutableParagraphStyle()
    style.alignment = .center
    let attributes: [NSAttributedString.Key: Any] = [
      .font: NSFont.systemFont(ofSize: size, weight: weight),
      .foregroundColor: NSColor(white: 1, alpha: alpha),
      .paragraphStyle: style,
      .kern: tracking,
    ]
    // AppKit lays text out bottom-up, so it is drawn in an un-flipped box at the
    // top-left `y` the layout asks for.
    let cg = NSGraphicsContext.current!.cgContext
    let box = size * 1.5
    cg.saveGState()
    cg.translateBy(x: 0, y: y + box)
    cg.scaleBy(x: 1, y: -1)
    let text = NSAttributedString(string: string, attributes: attributes)
    text.draw(with: NSRect(x: 0, y: 0, width: width, height: box), options: [.usesLineFragmentOrigin])
    cg.restoreGState()
  }
  draw("Install Parla", size: 22, weight: .semibold, alpha: 0.95, y: 46, tracking: -0.3)
  draw("Drag Parla into your Applications folder", size: 13, weight: .regular, alpha: 0.5, y: 78)
  draw("On-device dictation  ·  Your voice never leaves this Mac", size: 11, weight: .medium, alpha: 0.32, y: 334, tracking: 0.2)
}

let folder = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "."
render(scale: 1, to: "\(folder)/dmg-background.png")
render(scale: 2, to: "\(folder)/dmg-background@2x.png")
