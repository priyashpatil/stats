import AppKit
import SwiftTerm

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate,
  @preconcurrency LocalProcessTerminalViewDelegate
{
  private let frameAutosaveName = "StatsWindow"
  private var terminal: LocalProcessTerminalView?
  private var window: NSWindow?

  func applicationDidFinishLaunching(_ notification: Notification) {
    installMainMenu()

    let home = FileManager.default.homeDirectoryForCurrentUser.path
    let executable = "\(home)/.cargo/bin/stats"
    guard FileManager.default.isExecutableFile(atPath: executable) else {
      showMissingExecutableAlert(executable)
      NSApp.terminate(nil)
      return
    }

    let defaultFrame = NSRect(x: 0, y: 0, width: 500, height: 680)
    let window = NSWindow(
      contentRect: defaultFrame,
      styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
      backing: .buffered,
      defer: false
    )
    window.title = "Stats"
    window.titleVisibility = .hidden
    window.titlebarAppearsTransparent = true
    window.titlebarSeparatorStyle = .none
    window.isMovableByWindowBackground = true
    window.standardWindowButton(.closeButton)?.isHidden = true
    window.standardWindowButton(.miniaturizeButton)?.isHidden = true
    window.standardWindowButton(.zoomButton)?.isHidden = true
    window.delegate = self
    window.contentMinSize = NSSize(width: 500, height: 420)
    if !window.setFrameUsingName(frameAutosaveName) {
      window.center()
    }
    if window.frame.width < 500 {
      var frame = window.frame
      frame.size.width = 500
      window.setFrame(frame, display: false)
    }
    window.setFrameAutosaveName(frameAutosaveName)

    let background = NSColor(
      srgbRed: 0.0807,
      green: 0.0991,
      blue: 0.1210,
      alpha: 1
    )
    window.backgroundColor = background

    let terminalFrame = (window.contentView?.bounds ?? .zero).insetBy(dx: 8, dy: 8)
    let font =
      NSFont(name: "JetBrainsMonoNL-Regular", size: 15)
      ?? NSFont.monospacedSystemFont(ofSize: 15, weight: .regular)
    let terminal = LocalProcessTerminalView(
      frame: terminalFrame,
      font: font,
      options: .default
    )
    terminal.processDelegate = self
    terminal.autoresizingMask = [.width, .height]
    terminal.nativeBackgroundColor = background
    terminal.nativeForegroundColor = NSColor(
      srgbRed: 0.862,
      green: 0.862,
      blue: 0.862,
      alpha: 1
    )
    terminal.installColors(statsPalette)
    for scroller in terminal.subviews.compactMap({ $0 as? NSScroller }) {
      scroller.isHidden = true
    }
    window.contentView?.addSubview(terminal)

    self.window = window
    self.terminal = terminal

    window.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
    terminal.startProcess(
      executable: executable,
      environment: processEnvironment(home: home)
    )
  }

  func applicationWillTerminate(_ notification: Notification) {
    terminal?.terminate()
  }

  func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    true
  }

  func windowWillClose(_ notification: Notification) {
    terminal?.terminate()
  }

  func sizeChanged(source: LocalProcessTerminalView, newCols: Int, newRows: Int) {}

  func setTerminalTitle(source: LocalProcessTerminalView, title: String) {}

  func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {}

  func processTerminated(source: TerminalView, exitCode: Int32?) {
    NSApp.terminate(nil)
  }

  private func processEnvironment(home: String) -> [String] {
    var environment = ProcessInfo.processInfo.environment
    environment["HOME"] = home
    environment["TERM"] = "xterm-256color"
    environment["COLORTERM"] = "truecolor"
    environment["PATH"] = [
      "\(home)/.local/bin",
      "\(home)/.cargo/bin",
      "/opt/homebrew/bin",
      "/opt/homebrew/sbin",
      "/usr/local/bin",
      "/usr/bin",
      "/bin",
      "/usr/sbin",
      "/sbin",
    ].joined(separator: ":")
    return environment.map { "\($0.key)=\($0.value)" }
  }

  private var statsPalette: [SwiftTerm.Color] {
    [
      Color(red8: 20, green8: 25, blue8: 30),
      Color(red8: 180, green8: 60, blue8: 42),
      Color(red8: 0, green8: 194, blue8: 0),
      Color(red8: 199, green8: 196, blue8: 0),
      Color(red8: 39, green8: 68, blue8: 199),
      Color(red8: 192, green8: 64, blue8: 190),
      Color(red8: 0, green8: 197, blue8: 199),
      Color(red8: 199, green8: 199, blue8: 199),
      Color(red8: 104, green8: 104, blue8: 104),
      Color(red8: 221, green8: 121, blue8: 117),
      Color(red8: 88, green8: 231, blue8: 144),
      Color(red8: 236, green8: 225, blue8: 0),
      Color(red8: 167, green8: 171, blue8: 242),
      Color(red8: 225, green8: 126, blue8: 225),
      Color(red8: 96, green8: 253, blue8: 255),
      Color(red8: 255, green8: 255, blue8: 255),
    ]
  }

  private func installMainMenu() {
    let menu = NSMenu()
    let appMenuItem = NSMenuItem()
    menu.addItem(appMenuItem)

    let appMenu = NSMenu()
    appMenu.addItem(
      withTitle: "Quit Stats",
      action: #selector(NSApplication.terminate(_:)),
      keyEquivalent: "q"
    )
    appMenuItem.submenu = appMenu
    NSApp.mainMenu = menu
  }

  private func showMissingExecutableAlert(_ path: String) {
    let alert = NSAlert()
    alert.alertStyle = .critical
    alert.messageText = "Stats is not installed"
    alert.informativeText =
      "Expected to find the stats executable at \(path). Run install.sh and try again."
    alert.runModal()
  }
}

@main
struct StatsApplication {
  @MainActor
  static func main() {
    let application = NSApplication.shared
    let delegate = AppDelegate()
    application.delegate = delegate
    application.setActivationPolicy(.regular)
    application.run()
  }
}
