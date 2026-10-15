import AppKit
import SwiftTerm

final class DashboardTerminalView: LocalProcessTerminalView {
  override func setNeedsDisplay(_ invalidRect: NSRect) {
    // Repaint static rows too: after background/display transitions, partial
    // terminal updates can leave only the changing metrics visible. This small
    // dashboard can afford a full repaint at SwiftTerm's existing update cadence.
    super.setNeedsDisplay(bounds)
  }
}
