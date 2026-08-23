import AppKit

@MainActor
struct WindowPlacementStore {
  private let defaults: UserDefaults
  private let key = "mainWindowPlacement"

  init(defaults: UserDefaults = .standard) {
    self.defaults = defaults
  }

  func restore(_ window: NSWindow) {
    guard let screen = NSScreen.screens.first else { return }
    let visibleFrame = screen.visibleFrame
    let placement = defaults.dictionary(forKey: key)
    let width = placement?["width"] as? Double ?? window.frame.width
    let height = placement?["height"] as? Double ?? window.frame.height
    let x = placement?["x"] as? Double ?? (visibleFrame.width - width) / 2
    let top = placement?["top"] as? Double ?? (visibleFrame.height - height) / 2
    let frame = NSRect(
      x: visibleFrame.minX + x,
      y: visibleFrame.maxY - top - height,
      width: width,
      height: height
    )
    window.setFrame(window.constrainFrameRect(frame, to: screen), display: false)
  }

  func save(_ window: NSWindow?) {
    guard
      let window,
      let screen = window.screen,
      let primaryScreen = NSScreen.screens.first,
      screen == primaryScreen
    else {
      return
    }
    let frame = window.frame
    let visibleFrame = screen.visibleFrame
    defaults.set(
      [
        "x": frame.minX - visibleFrame.minX,
        "top": visibleFrame.maxY - frame.maxY,
        "width": frame.width,
        "height": frame.height,
      ],
      forKey: key
    )
  }
}
