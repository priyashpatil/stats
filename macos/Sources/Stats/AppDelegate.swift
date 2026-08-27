import AppKit
import SwiftTerm

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate,
  NSMenuDelegate, @preconcurrency LocalProcessTerminalViewDelegate
{
  private let launchAtLoginController = LaunchAtLoginController()
  private let windowPlacementStore = WindowPlacementStore()
  private var configStore: StatsConfigStore?
  private var executable: String?
  private var home: String?
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

    let configStore: StatsConfigStore
    do {
      configStore = try StatsConfigStore()
    } catch {
      showConfigErrorAlert(error)
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
    window.contentMinSize = NSSize(width: defaultWidth, height: 80)
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

    let contentBounds = window.contentView?.bounds ?? .zero
    let terminalFrame = dashboardTerminalFrame(
      in: contentBounds,
      showsScrollbar: configStore.config.desktop.showScrollbar
    )
    let font = terminalFont(size: configStore.config.desktop.fontSize)
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
    self.configStore = configStore
    self.executable = executable
    self.home = home

    showMainWindow(window)
    startTerminalProcess()
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

  func setTerminalTitle(source: LocalProcessTerminalView, title: String) {
    let prefix = "stats-layout:"
    guard
      title.hasPrefix(prefix),
      let rows = Int(title.dropFirst(prefix.count)),
      rows > 0
    else { return }
    resizeWindowToFit(source, rows: rows)
  }

  func hostCurrentDirectoryUpdate(source: TerminalView, directory: String?) {}

  func processTerminated(source: TerminalView, exitCode: Int32?) {
    NSApp.terminate(nil)
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
    guard let configStore else { return }
    if let settingsWindowController {
      settingsWindowController.showWindow(nil)
      settingsWindowController.window?.makeKeyAndOrderFront(nil)
      NSApp.activate(ignoringOtherApps: true)
      return
    }
    let controller = SettingsWindowController(
      selectedClockChoices: configStore.config.clocks,
      launchesAtLogin: launchAtLoginController.isEnabled,
      fontSize: configStore.config.desktop.fontSize,
      showsScrollbar: configStore.config.desktop.showScrollbar,
      sections: configStore.config.sections,
      sectionDisplay: configStore.config.sectionDisplay,
      configPath: configStore.url.path,
      onLaunchAtLoginChange: { [launchAtLoginController] enabled in
        launchAtLoginController.setEnabled(enabled)
      },
      onFontSizeChange: { [weak self] fontSize in
        guard let self else { return }
        do {
          try configStore.saveFontSize(fontSize)
          self.terminal?.font = self.terminalFont(size: fontSize)
        } catch {
          self.showConfigErrorAlert(error)
        }
      },
      onShowScrollbarChange: { [weak self] showScrollbar in
        guard let self else { return false }
        do {
          try configStore.saveShowScrollbar(showScrollbar)
          self.layoutTerminal(showsScrollbar: showScrollbar)
          return true
        } catch {
          self.showConfigErrorAlert(error)
          return false
        }
      },
      onSectionSettingsChange: { [weak self] sections, display in
        guard let self else { return false }
        do {
          try configStore.saveSectionSettings(sections, display: display)
          return true
        } catch {
          self.showConfigErrorAlert(error)
          return false
        }
      },
      onClockChoicesChange: { [weak self] choices in
        guard let self else { return }
        do {
          try configStore.saveClocks(choices)
        } catch {
          self.showConfigErrorAlert(error)
        }
      },
      onOpenConfig: { [weak self] in
        guard let self else { return }
        do {
          try configStore.ensureFileExists()
          NSWorkspace.shared.open(configStore.url)
        } catch {
          self.showConfigErrorAlert(error)
        }
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

  private func dashboardTerminalFrame(
    in bounds: NSRect,
    showsScrollbar: Bool
  ) -> NSRect {
    let rightInset: CGFloat = showsScrollbar ? 0 : 16
    return NSRect(
      x: bounds.minX + 16,
      y: bounds.minY + 16,
      width: max(0, bounds.width - 16 - rightInset),
      height: max(0, bounds.height - 32)
    )
  }

  private func layoutTerminal(showsScrollbar: Bool) {
    guard let terminal, let bounds = window?.contentView?.bounds else { return }
    terminal.frame = dashboardTerminalFrame(in: bounds, showsScrollbar: showsScrollbar)
    terminal.setFrameSize(terminal.frame.size)
  }

  private func processEnvironment(home: String) -> [String] {
    var environment = ProcessInfo.processInfo.environment
    environment.removeValue(forKey: "NO_COLOR")
    environment["HOME"] = home
    environment["STATS_DESKTOP_LAYOUT"] = "1"
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

  private func startTerminalProcess() {
    guard let terminal, let executable, let home, let configStore else { return }
    terminal.startProcess(
      executable: executable,
      args: ["--config", configStore.url.path],
      environment: processEnvironment(home: home)
    )
  }

  private func resizeWindowToFit(_ terminal: LocalProcessTerminalView, rows: Int) {
    guard
      let window,
      let contentView = window.contentView,
      let screen = window.screen ?? NSScreen.screens.first
    else { return }
    let terminalRows = terminal.getTerminal().rows
    guard terminalRows > 0 else { return }

    let optimalTerminalFrame = terminal.getOptimalFrameSize()
    let rowHeight = optimalTerminalFrame.height / CGFloat(terminalRows)
    let verticalInsets = contentView.bounds.height - terminal.frame.height
    let targetContentHeight = max(
      window.contentMinSize.height,
      CGFloat(rows) * rowHeight + verticalInsets
    )
    let contentRect = NSRect(
      x: 0,
      y: 0,
      width: contentView.bounds.width,
      height: targetContentHeight
    )
    let requestedFrame = window.frameRect(forContentRect: contentRect)
    let targetHeight = min(requestedFrame.height, screen.visibleFrame.height)
    guard abs(window.frame.height - targetHeight) >= rowHeight / 2 else { return }

    var frame = window.frame
    frame.origin.y = frame.maxY - targetHeight
    frame.size.height = targetHeight
    frame = window.constrainFrameRect(frame, to: screen)
    window.setFrame(frame, display: true, animate: false)
    windowPlacementStore.save(window)
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

  private func showConfigErrorAlert(_ error: Error) {
    let alert = NSAlert()
    alert.alertStyle = .critical
    alert.messageText = "Stats configuration error"
    alert.informativeText = "\(error.localizedDescription)"
    alert.runModal()
  }
}
