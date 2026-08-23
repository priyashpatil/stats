import AppKit
import SwiftTerm

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate,
  NSMenuDelegate, @preconcurrency LocalProcessTerminalViewDelegate
{
  private let clockPreferences = ClockPreferences()
  private let launchAtLoginController = LaunchAtLoginController()
  private let terminalPreferences = TerminalPreferences()
  private let windowPlacementStore = WindowPlacementStore()
  private var executable: String?
  private var home: String?
  private var isRestartingTerminal = false
  private var aboutWindowController: NSWindowController?
  private var settingsWindowController: SettingsWindowController?
  private var showHideMenuItem: NSMenuItem?
  private var statusItem: NSStatusItem?
  private var terminal: LocalProcessTerminalView?
  private var window: NSWindow?

  func applicationDidFinishLaunching(_ notification: Notification) {
    installMainMenu()
    installStatusItem()

    let home = FileManager.default.homeDirectoryForCurrentUser.path
    let installedExecutable = "\(home)/.cargo/bin/stats"
    let executableCandidates = [
      Bundle.main.resourceURL?.appendingPathComponent("stats").path,
      installedExecutable,
    ].compactMap { $0 }
    guard
      let executable = executableCandidates.first(where: {
        FileManager.default.isExecutableFile(atPath: $0)
      })
    else {
      showMissingExecutableAlert(installedExecutable)
      NSApp.terminate(nil)
      return
    }

    let defaultWidth: CGFloat = 450
    let defaultFrame = NSRect(x: 0, y: 0, width: defaultWidth, height: 646)
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
    window.collectionBehavior = [.managed, .fullScreenNone]
    window.contentMinSize = NSSize(width: defaultWidth, height: 420)
    windowPlacementStore.restore(window)
    if window.frame.width < defaultWidth {
      var frame = window.frame
      frame.size.width = defaultWidth
      window.setFrame(frame, display: false)
    }

    let background = NSColor(
      srgbRed: 0.0807,
      green: 0.0991,
      blue: 0.1210,
      alpha: 1
    )
    window.backgroundColor = background

    let terminalFrame = (window.contentView?.bounds ?? .zero).insetBy(dx: 16, dy: 16)
    let font = terminalFont(size: terminalPreferences.fontSize)
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
    terminal.setFrameSize(terminal.frame.size)
    window.contentView?.addSubview(terminal)

    self.window = window
    self.terminal = terminal
    self.executable = executable
    self.home = home

    showMainWindow(window)
    terminal.startProcess(
      executable: executable,
      environment: processEnvironment(home: home)
    )
  }

  func applicationWillTerminate(_ notification: Notification) {
    windowPlacementStore.save(window)
    terminal?.terminate()
  }

  func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    false
  }

  func windowShouldClose(_ sender: NSWindow) -> Bool {
    sender.orderOut(nil)
    return false
  }

  func windowDidMove(_ notification: Notification) {
    windowPlacementStore.save(window)
  }

  func windowDidResize(_ notification: Notification) {
    windowPlacementStore.save(window)
  }

  func windowDidChangeScreen(_ notification: Notification) {
    windowPlacementStore.save(window)
  }

  func sizeChanged(source: LocalProcessTerminalView, newCols: Int, newRows: Int) {}

  func setTerminalTitle(source: LocalProcessTerminalView, title: String) {}

  func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {}

  func processTerminated(source: TerminalView, exitCode: Int32?) {
    if !isRestartingTerminal {
      NSApp.terminate(nil)
    }
  }

  func menuWillOpen(_ menu: NSMenu) {
    showHideMenuItem?.title = window?.isVisible == true ? "Hide Stats" : "Show Stats"
  }

  @objc private func toggleWindow(_ sender: Any?) {
    guard let window else { return }
    if window.isVisible {
      window.orderOut(nil)
    } else {
      showMainWindow(window)
    }
  }

  @objc private func showAboutWindow(_ sender: Any?) {
    if aboutWindowController == nil {
      let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 520, height: 400),
        styleMask: [.titled, .closable],
        backing: .buffered,
        defer: false
      )
      window.title = "About Stats"
      window.isReleasedWhenClosed = false
      window.contentViewController = AboutViewController()
      window.center()
      aboutWindowController = NSWindowController(window: window)
    }
    aboutWindowController?.showWindow(nil)
    aboutWindowController?.window?.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
  }

  @objc private func showSettings(_ sender: Any?) {
    if let settingsWindowController {
      settingsWindowController.showWindow(nil)
      settingsWindowController.window?.makeKeyAndOrderFront(nil)
      NSApp.activate(ignoringOtherApps: true)
      return
    }
    let controller = SettingsWindowController(
      selectedClockChoices: clockPreferences.selectedChoices,
      launchesAtLogin: launchAtLoginController.isEnabled,
      fontSize: terminalPreferences.fontSize,
      onLaunchAtLoginChange: { [launchAtLoginController] enabled in
        launchAtLoginController.setEnabled(enabled)
      },
      onFontSizeChange: { [weak self] fontSize in
        guard let self else { return }
        self.terminalPreferences.saveFontSize(fontSize)
        self.terminal?.font = self.terminalFont(size: fontSize)
      },
      onClockChoicesChange: { [weak self] choices in
        guard let self else { return }
        self.clockPreferences.save(choices)
        self.restartTerminal()
      }
    )
    settingsWindowController = controller
    controller.showWindow(nil)
    controller.window?.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
  }

  private func terminalFont(size: Int) -> NSFont {
    NSFont(name: "JetBrainsMonoNL-Regular", size: CGFloat(size))
      ?? NSFont.monospacedSystemFont(ofSize: CGFloat(size), weight: .regular)
  }

  private func processEnvironment(home: String) -> [String] {
    var environment = ProcessInfo.processInfo.environment
    environment.removeValue(forKey: "NO_COLOR")
    environment["HOME"] = home
    environment["TERM"] = "xterm-256color"
    environment["COLORTERM"] = "truecolor"
    if let data = try? JSONEncoder().encode(clockPreferences.selectedChoices),
      let value = String(data: data, encoding: .utf8)
    {
      environment["STATS_CLOCKS"] = value
    }
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

  private func restartTerminal() {
    guard let terminal, let executable, let home else { return }
    isRestartingTerminal = true
    terminal.terminate()
    terminal.startProcess(
      executable: executable,
      environment: processEnvironment(home: home)
    )
    DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
      self?.isRestartingTerminal = false
    }
  }

  private func showMainWindow(_ window: NSWindow) {
    windowPlacementStore.restore(window)
    window.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
    DispatchQueue.main.async { [weak self, weak window] in
      guard let self, let window else { return }
      self.windowPlacementStore.restore(window)
      window.makeKeyAndOrderFront(nil)
      self.windowPlacementStore.save(window)
    }
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
    let settingsItem = appMenu.addItem(
      withTitle: "Settings…",
      action: #selector(showSettings(_:)),
      keyEquivalent: ","
    )
    settingsItem.target = self
    appMenu.addItem(.separator())
    appMenu.addItem(
      withTitle: "Quit Stats",
      action: #selector(NSApplication.terminate(_:)),
      keyEquivalent: "q"
    )
    appMenuItem.submenu = appMenu
    NSApp.mainMenu = menu
  }

  private func installStatusItem() {
    let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    if let button = statusItem.button {
      button.image =
        NSImage(
          systemSymbolName: "waveform.path.ecg.text",
          accessibilityDescription: "Stats"
        )
        ?? NSImage(systemSymbolName: "waveform.path.ecg", accessibilityDescription: "Stats")
    }

    let menu = NSMenu()
    menu.delegate = self
    let showHideItem = NSMenuItem(
      title: "Hide Stats",
      action: #selector(toggleWindow(_:)),
      keyEquivalent: ""
    )
    showHideItem.target = self
    menu.addItem(showHideItem)
    let settingsItem = NSMenuItem(
      title: "Settings…",
      action: #selector(showSettings(_:)),
      keyEquivalent: ","
    )
    settingsItem.target = self
    menu.addItem(settingsItem)
    menu.addItem(.separator())
    let aboutItem = NSMenuItem(
      title: "About Stats",
      action: #selector(showAboutWindow(_:)),
      keyEquivalent: ""
    )
    aboutItem.target = self
    menu.addItem(aboutItem)
    menu.addItem(
      withTitle: "Quit Stats",
      action: #selector(NSApplication.terminate(_:)),
      keyEquivalent: "q"
    )
    statusItem.menu = menu

    self.showHideMenuItem = showHideItem
    self.statusItem = statusItem
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
