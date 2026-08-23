import AppKit
import SwiftTerm

private struct ClockChoice: Codable {
  let label: String
  let timezone: String

  var id: String { "\(label)|\(timezone)" }

  static let defaults = [
    ClockChoice(label: "Mumbai", timezone: "Asia/Kolkata"),
    ClockChoice(label: "Paris", timezone: "Europe/Paris"),
    ClockChoice(label: "Sydney", timezone: "Australia/Sydney"),
    ClockChoice(label: "Seattle", timezone: "America/Los_Angeles"),
  ]

  static let all: [ClockChoice] = {
    let aliases = [
      ClockChoice(label: "Abu Dhabi", timezone: "Asia/Dubai"),
      ClockChoice(label: "Atlanta", timezone: "America/New_York"),
      ClockChoice(label: "Austin", timezone: "America/Chicago"),
      ClockChoice(label: "Barcelona", timezone: "Europe/Madrid"),
      ClockChoice(label: "Beijing", timezone: "Asia/Shanghai"),
      ClockChoice(label: "Bengaluru", timezone: "Asia/Kolkata"),
      ClockChoice(label: "Boston", timezone: "America/New_York"),
      ClockChoice(label: "Brasília", timezone: "America/Sao_Paulo"),
      ClockChoice(label: "Canberra", timezone: "Australia/Sydney"),
      ClockChoice(label: "Cape Town", timezone: "Africa/Johannesburg"),
      ClockChoice(label: "Chennai", timezone: "Asia/Kolkata"),
      ClockChoice(label: "Dallas", timezone: "America/Chicago"),
      ClockChoice(label: "Delhi", timezone: "Asia/Kolkata"),
      ClockChoice(label: "Guangzhou", timezone: "Asia/Shanghai"),
      ClockChoice(label: "Houston", timezone: "America/Chicago"),
      ClockChoice(label: "Hyderabad", timezone: "Asia/Kolkata"),
      ClockChoice(label: "Islamabad", timezone: "Asia/Karachi"),
      ClockChoice(label: "Lahore", timezone: "Asia/Karachi"),
      ClockChoice(label: "Las Vegas", timezone: "America/Los_Angeles"),
      ClockChoice(label: "Los Angeles", timezone: "America/Los_Angeles"),
      ClockChoice(label: "Miami", timezone: "America/New_York"),
      ClockChoice(label: "Milan", timezone: "Europe/Rome"),
      ClockChoice(label: "Mumbai", timezone: "Asia/Kolkata"),
      ClockChoice(label: "Munich", timezone: "Europe/Berlin"),
      ClockChoice(label: "Osaka", timezone: "Asia/Tokyo"),
      ClockChoice(label: "Philadelphia", timezone: "America/New_York"),
      ClockChoice(label: "Portland", timezone: "America/Los_Angeles"),
      ClockChoice(label: "Rio de Janeiro", timezone: "America/Sao_Paulo"),
      ClockChoice(label: "San Diego", timezone: "America/Los_Angeles"),
      ClockChoice(label: "San Francisco", timezone: "America/Los_Angeles"),
      ClockChoice(label: "Seattle", timezone: "America/Los_Angeles"),
      ClockChoice(label: "Shenzhen", timezone: "Asia/Shanghai"),
      ClockChoice(label: "São Paulo", timezone: "America/Sao_Paulo"),
      ClockChoice(label: "Washington, D.C.", timezone: "America/New_York"),
      ClockChoice(label: "Wellington", timezone: "Pacific/Auckland"),
    ]
    let geographicPrefixes = [
      "Africa/", "America/", "Antarctica/", "Arctic/", "Asia/", "Atlantic/",
      "Australia/", "Europe/", "Indian/", "Pacific/",
    ]
    let timezoneCities = TimeZone.knownTimeZoneIdentifiers.compactMap { timezone -> ClockChoice? in
      guard geographicPrefixes.contains(where: { timezone.hasPrefix($0) }),
        let city = timezone.split(separator: "/").last
      else {
        return nil
      }
      return ClockChoice(
        label: city.replacingOccurrences(of: "_", with: " "),
        timezone: timezone
      )
    }
    var seen: Set<String> = []
    return (aliases + timezoneCities)
      .filter {
        let id = $0.id.folding(
          options: [.caseInsensitive, .diacriticInsensitive],
          locale: Locale(identifier: "en_US_POSIX")
        )
        return seen.insert(id).inserted
      }
      .sorted {
        let comparison = $0.label.localizedStandardCompare($1.label)
        return comparison == .orderedSame
          ? $0.timezone < $1.timezone : comparison == .orderedAscending
      }
  }()

  static func choice(forLegacyTimezone timezone: String) -> ClockChoice? {
    let legacyLabels = [
      "Africa/Johannesburg": "Cape Town",
      "America/Chicago": "Chicago",
      "America/Los_Angeles": "Seattle",
      "America/New_York": "New York",
      "America/Sao_Paulo": "São Paulo",
      "Asia/Dubai": "Dubai",
      "Asia/Karachi": "Karachi",
      "Asia/Kolkata": "Mumbai",
      "Asia/Shanghai": "Shanghai",
      "Asia/Tokyo": "Tokyo",
      "Australia/Sydney": "Sydney",
      "Europe/Berlin": "Berlin",
      "Europe/Madrid": "Madrid",
      "Europe/Rome": "Rome",
      "Pacific/Auckland": "Auckland",
    ]
    if let label = legacyLabels[timezone],
      let choice = all.first(where: { $0.label == label && $0.timezone == timezone })
    {
      return choice
    }
    return all.first(where: { $0.timezone == timezone })
  }

  func title(at date: Date) -> String {
    let formatter = DateFormatter()
    formatter.locale = Locale.current
    formatter.dateFormat = "EEE h:mm a"
    formatter.timeZone = TimeZone(identifier: timezone)
    return "\(label) — \(formatter.string(from: date)) — \(timezone)"
  }
}

@MainActor
private final class AboutViewController: NSViewController {
  override func loadView() {
    preferredContentSize = NSSize(width: 520, height: 400)
    view = NSView(frame: NSRect(origin: .zero, size: preferredContentSize))

    let icon = NSImageView()
    icon.image = NSApp.applicationIconImage
    icon.imageScaling = .scaleProportionallyUpOrDown
    icon.translatesAutoresizingMaskIntoConstraints = false
    NSLayoutConstraint.activate([
      icon.widthAnchor.constraint(equalToConstant: 96),
      icon.heightAnchor.constraint(equalToConstant: 96),
    ])

    let name = NSTextField(labelWithString: "Stats")
    name.font = .systemFont(ofSize: 24, weight: .semibold)
    let version =
      Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
      ?? "Development"
    let versionLabel = NSTextField(labelWithString: "Version \(version)")
    versionLabel.textColor = .secondaryLabelColor
    let description = NSTextField(
      wrappingLabelWithString: "A terminal dashboard for macOS system metrics, Amp usage, and Codex usage."
    )
    description.alignment = .center
    description.textColor = .secondaryLabelColor
    description.maximumNumberOfLines = 2
    description.preferredMaxLayoutWidth = 360
    description.widthAnchor.constraint(equalToConstant: 360).isActive = true

    let changelogButton = NSButton(
      title: "Changelog",
      target: self,
      action: #selector(openChangelog(_:))
    )
    changelogButton.bezelStyle = .rounded

    let stack = NSStackView(views: [icon, name, versionLabel, description, changelogButton])
    stack.orientation = .vertical
    stack.alignment = .centerX
    stack.spacing = 8
    stack.setCustomSpacing(14, after: versionLabel)
    stack.setCustomSpacing(16, after: description)
    stack.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.centerXAnchor.constraint(equalTo: view.centerXAnchor),
      stack.topAnchor.constraint(equalTo: view.topAnchor, constant: 48),
    ])
  }

  @objc private func openChangelog(_ sender: NSButton) {
    NSWorkspace.shared.open(URL(string: "https://github.com/priyashpatil/stats/releases")!)
  }
}

@MainActor
private final class ClockPickerButton: NSPopUpButton {
  var onOpen: ((ClockPickerButton) -> Void)?

  override func mouseDown(with event: NSEvent) {
    onOpen?(self)
  }
}

@MainActor
private final class ClockPickerViewController: NSViewController, NSTableViewDataSource,
  NSTableViewDelegate, NSSearchFieldDelegate
{
  private let choices = ClockChoice.all
  private var filteredChoices = ClockChoice.all
  private let onSelect: (ClockChoice) -> Void
  private let searchField = NSSearchField()
  private let tableView = NSTableView()
  private let titles = ClockChoice.all.map { $0.title(at: Date()) }

  init(onSelect: @escaping (ClockChoice) -> Void) {
    self.onSelect = onSelect
    super.init(nibName: nil, bundle: nil)
  }

  required init?(coder: NSCoder) {
    fatalError("init(coder:) has not been implemented")
  }

  override func loadView() {
    view = NSView(frame: NSRect(x: 0, y: 0, width: 360, height: 320))

    searchField.placeholderString = "Search cities or time zones"
    searchField.delegate = self
    searchField.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(searchField)

    let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("ClockCity"))
    column.width = 344
    tableView.addTableColumn(column)
    tableView.headerView = nil
    tableView.rowHeight = 26
    tableView.intercellSpacing = NSSize(width: 0, height: 0)
    tableView.dataSource = self
    tableView.delegate = self
    tableView.target = self
    tableView.action = #selector(selectTableRow(_:))

    let scrollView = NSScrollView()
    scrollView.documentView = tableView
    scrollView.hasVerticalScroller = true
    scrollView.drawsBackground = false
    scrollView.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(scrollView)

    NSLayoutConstraint.activate([
      searchField.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 8),
      searchField.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -8),
      searchField.topAnchor.constraint(equalTo: view.topAnchor, constant: 8),
      scrollView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
      scrollView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
      scrollView.topAnchor.constraint(equalTo: searchField.bottomAnchor, constant: 6),
      scrollView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
    ])
  }

  override func viewDidAppear() {
    super.viewDidAppear()
    view.window?.makeFirstResponder(searchField)
  }

  func numberOfRows(in tableView: NSTableView) -> Int {
    filteredChoices.count
  }

  func tableView(
    _ tableView: NSTableView,
    viewFor tableColumn: NSTableColumn?,
    row: Int
  ) -> NSView? {
    let choice = filteredChoices[row]
    let label = NSTextField(labelWithString: title(for: choice))
    label.lineBreakMode = .byTruncatingTail
    label.translatesAutoresizingMaskIntoConstraints = false
    let cell = NSTableCellView()
    cell.addSubview(label)
    NSLayoutConstraint.activate([
      label.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 8),
      label.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -8),
      label.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
    ])
    return cell
  }

  func controlTextDidChange(_ notification: Notification) {
    let query = normalized(searchField.stringValue)
    filteredChoices = choices.filter {
      query.isEmpty || normalized($0.label).contains(query)
        || normalized($0.timezone).contains(query)
    }
    tableView.reloadData()
  }

  func control(
    _ control: NSControl,
    textView: NSTextView,
    doCommandBy commandSelector: Selector
  ) -> Bool {
    if commandSelector == #selector(NSResponder.moveDown(_:)) {
      moveSelection(by: 1)
      return true
    }
    if commandSelector == #selector(NSResponder.moveUp(_:)) {
      moveSelection(by: -1)
      return true
    }
    if commandSelector == #selector(NSResponder.insertNewline(_:))
      || commandSelector == #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:))
    {
      commitSelectedChoice()
      return true
    }
    if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
      view.window?.close()
      return true
    }
    return false
  }

  @objc private func selectTableRow(_ sender: NSTableView) {
    commitSelectedChoice()
  }

  private func commitSelectedChoice() {
    let row = tableView.selectedRow >= 0 ? tableView.selectedRow : 0
    guard filteredChoices.indices.contains(row) else { return }
    onSelect(filteredChoices[row])
  }

  private func moveSelection(by offset: Int) {
    guard !filteredChoices.isEmpty else { return }
    let currentRow = tableView.selectedRow
    let nextRow =
      currentRow < 0
      ? (offset > 0 ? 0 : filteredChoices.count - 1)
      : min(max(currentRow + offset, 0), filteredChoices.count - 1)
    tableView.selectRowIndexes(IndexSet(integer: nextRow), byExtendingSelection: false)
    tableView.scrollRowToVisible(nextRow)
  }

  private func title(for choice: ClockChoice) -> String {
    guard let index = choices.firstIndex(where: { $0.id == choice.id }) else { return choice.label }
    return titles[index]
  }

  private func normalized(_ value: String) -> String {
    value.folding(
      options: [.caseInsensitive, .diacriticInsensitive],
      locale: Locale.current
    )
  }
}

@MainActor
private final class ClockPickerPanel: NSPanel {
  override var canBecomeKey: Bool { true }

  override func resignKey() {
    super.resignKey()
    close()
  }
}

@MainActor
private final class SettingsWindowController: NSWindowController, NSTableViewDataSource,
  NSTableViewDelegate
{
  private let detailController = NSViewController()
  private var panes: [NSViewController] = []
  private let sidebarItems = [
    (title: "General", symbol: "gearshape"),
    (title: "Clocks", symbol: "clock"),
    (title: "About", symbol: "info.circle"),
  ]
  private let sidebarTable = NSTableView()
  private let clockMenuWidth: CGFloat = 360
  private var clockPickerPanel: ClockPickerPanel?
  private var selectedClockChoices: [ClockChoice]
  private var timezonePopups: [ClockPickerButton] = []
  private let onLaunchAtLoginChange: (Bool) -> Bool
  private let onClockChoicesChange: ([ClockChoice]) -> Void

  init(
    selectedClockChoices: [ClockChoice],
    launchesAtLogin: Bool,
    onLaunchAtLoginChange: @escaping (Bool) -> Bool,
    onClockChoicesChange: @escaping ([ClockChoice]) -> Void
  ) {
    self.selectedClockChoices = selectedClockChoices
    self.onLaunchAtLoginChange = onLaunchAtLoginChange
    self.onClockChoicesChange = onClockChoicesChange
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 720, height: 440),
      styleMask: [.titled, .closable, .resizable],
      backing: .buffered,
      defer: false
    )
    super.init(window: window)
    window.title = "Stats Settings"
    window.isReleasedWhenClosed = false
    window.center()
    configureContent(selectedClockChoices: selectedClockChoices, launchesAtLogin: launchesAtLogin)
  }

  required init?(coder: NSCoder) {
    fatalError("init(coder:) has not been implemented")
  }

  private func configureContent(selectedClockChoices: [ClockChoice], launchesAtLogin: Bool) {
    panes = [
      generalViewController(launchesAtLogin: launchesAtLogin),
      clocksViewController(selectedClockChoices: selectedClockChoices),
      AboutViewController(),
    ]

    let sidebarController = NSViewController()
    let sidebar = NSVisualEffectView()
    sidebar.material = .sidebar
    sidebar.blendingMode = .behindWindow
    sidebar.state = .active
    sidebarController.view = sidebar

    let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("SettingsSidebar"))
    sidebarTable.addTableColumn(column)
    sidebarTable.headerView = nil
    sidebarTable.rowHeight = 32
    sidebarTable.intercellSpacing = NSSize(width: 0, height: 2)
    sidebarTable.style = .sourceList
    sidebarTable.selectionHighlightStyle = .sourceList
    sidebarTable.dataSource = self
    sidebarTable.delegate = self
    sidebarTable.backgroundColor = .clear

    let appIcon = NSImageView()
    appIcon.image = NSApp.applicationIconImage
    appIcon.imageScaling = .scaleProportionallyUpOrDown
    let appName = NSTextField(labelWithString: "Stats")
    appName.font = .systemFont(ofSize: 15, weight: .semibold)
    let appHeader = NSStackView(views: [appIcon, appName])
    appHeader.orientation = .horizontal
    appHeader.alignment = .centerY
    appHeader.spacing = 8
    appHeader.translatesAutoresizingMaskIntoConstraints = false
    sidebar.addSubview(appHeader)

    let separator = NSBox()
    separator.boxType = .separator
    separator.translatesAutoresizingMaskIntoConstraints = false
    sidebar.addSubview(separator)

    let scrollView = NSScrollView()
    scrollView.drawsBackground = false
    scrollView.documentView = sidebarTable
    scrollView.translatesAutoresizingMaskIntoConstraints = false
    sidebar.addSubview(scrollView)
    NSLayoutConstraint.activate([
      appHeader.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor, constant: 12),
      appHeader.topAnchor.constraint(equalTo: sidebar.topAnchor, constant: 16),
      appIcon.widthAnchor.constraint(equalToConstant: 28),
      appIcon.heightAnchor.constraint(equalToConstant: 28),
      separator.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor, constant: 12),
      separator.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: -12),
      separator.topAnchor.constraint(equalTo: appHeader.bottomAnchor, constant: 12),
      scrollView.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor),
      scrollView.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor),
      scrollView.topAnchor.constraint(equalTo: separator.bottomAnchor, constant: 8),
      scrollView.bottomAnchor.constraint(equalTo: sidebar.bottomAnchor, constant: -12),
    ])

    detailController.view = NSView()
    let splitController = NSSplitViewController()
    let sidebarItem = NSSplitViewItem(sidebarWithViewController: sidebarController)
    sidebarItem.canCollapse = false
    sidebarItem.minimumThickness = 180
    sidebarItem.maximumThickness = 220
    splitController.addSplitViewItem(sidebarItem)
    splitController.addSplitViewItem(NSSplitViewItem(viewController: detailController))
    window?.contentViewController = splitController
    window?.contentMinSize = NSSize(width: 620, height: 360)

    sidebarTable.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
    showPane(at: 0)
  }

  private func generalViewController(launchesAtLogin: Bool) -> NSViewController {
    let controller = settingsPane()
    let title = NSTextField(labelWithString: "General")
    title.font = .systemFont(ofSize: 22, weight: .semibold)
    let checkbox = NSButton(
      checkboxWithTitle: "Launch Stats at login",
      target: self,
      action: #selector(launchAtLoginChanged(_:))
    )
    checkbox.state = launchesAtLogin ? .on : .off
    let help = NSTextField(
      wrappingLabelWithString: "Automatically open Stats when you sign in to your Mac."
    )
    help.textColor = .secondaryLabelColor

    let stack = NSStackView(views: [title, checkbox, help])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 6
    stack.setCustomSpacing(24, after: title)
    install(stack, in: controller.view)
    return controller
  }

  private func clocksViewController(selectedClockChoices: [ClockChoice]) -> NSViewController {
    let controller = settingsPane()

    let title = NSTextField(labelWithString: "Clocks")
    title.font = .systemFont(ofSize: 22, weight: .semibold)
    let help = NSTextField(
      wrappingLabelWithString:
        "Type a city or time zone in any clock field, then choose a matching city."
    )
    help.textColor = .secondaryLabelColor

    var rows: [[NSView]] = []
    for slot in 0..<4 {
      let label = NSTextField(labelWithString: "Clock \(slot + 1):")
      label.alignment = .left
      let popup = ClockPickerButton(frame: .zero, pullsDown: false)
      popup.addItem(withTitle: selectedClockChoices[slot].title(at: Date()))
      popup.onOpen = { [weak self] popup in
        self?.showClockPicker(for: popup, slot: slot)
      }
      popup.cell?.lineBreakMode = .byTruncatingTail
      popup.widthAnchor.constraint(equalToConstant: clockMenuWidth).isActive = true
      timezonePopups.append(popup)
      rows.append([label, popup])
    }

    let grid = NSGridView(views: rows)
    grid.rowSpacing = 12
    grid.columnSpacing = 12
    grid.column(at: 0).xPlacement = .leading
    grid.column(at: 0).width = 72
    grid.column(at: 1).xPlacement = .fill
    grid.column(at: 1).width = 360

    let stack = NSStackView(views: [title, help, grid])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 10
    stack.setCustomSpacing(8, after: title)
    stack.setCustomSpacing(20, after: help)
    install(stack, in: controller.view)
    return controller
  }

  private func settingsPane() -> NSViewController {
    let controller = NSViewController()
    controller.preferredContentSize = NSSize(width: 520, height: 400)
    controller.view = NSView(frame: NSRect(origin: .zero, size: controller.preferredContentSize))
    return controller
  }

  private func install(_ stack: NSStackView, in view: NSView) {
    stack.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 28),
      stack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -28),
      stack.topAnchor.constraint(equalTo: view.topAnchor, constant: 28),
    ])
  }

  func numberOfRows(in tableView: NSTableView) -> Int {
    sidebarItems.count
  }

  func tableView(
    _ tableView: NSTableView,
    viewFor tableColumn: NSTableColumn?,
    row: Int
  ) -> NSView? {
    let item = sidebarItems[row]
    let cell = NSTableCellView()
    let image = NSImageView()
    image.image = NSImage(systemSymbolName: item.symbol, accessibilityDescription: item.title)
    image.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 14, weight: .regular)
    let label = NSTextField(labelWithString: item.title)
    let stack = NSStackView(views: [image, label])
    stack.orientation = .horizontal
    stack.alignment = .centerY
    stack.spacing = 8
    stack.translatesAutoresizingMaskIntoConstraints = false
    cell.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 6),
      stack.trailingAnchor.constraint(lessThanOrEqualTo: cell.trailingAnchor, constant: -6),
      stack.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
      image.widthAnchor.constraint(equalToConstant: 18),
    ])
    return cell
  }

  func tableViewSelectionDidChange(_ notification: Notification) {
    let row = sidebarTable.selectedRow
    if row >= 0 {
      showPane(at: row)
    }
  }

  private func showPane(at index: Int) {
    for child in detailController.children {
      child.view.removeFromSuperview()
      child.removeFromParent()
    }
    let pane = panes[index]
    detailController.addChild(pane)
    pane.view.translatesAutoresizingMaskIntoConstraints = false
    detailController.view.addSubview(pane.view)
    NSLayoutConstraint.activate([
      pane.view.leadingAnchor.constraint(equalTo: detailController.view.leadingAnchor),
      pane.view.trailingAnchor.constraint(equalTo: detailController.view.trailingAnchor),
      pane.view.topAnchor.constraint(equalTo: detailController.view.topAnchor),
      pane.view.bottomAnchor.constraint(equalTo: detailController.view.bottomAnchor),
    ])
  }

  @objc private func launchAtLoginChanged(_ sender: NSButton) {
    let requested = sender.state == .on
    sender.state = onLaunchAtLoginChange(requested) ? .on : .off
  }

  private func showClockPicker(for popup: ClockPickerButton, slot: Int) {
    let picker = ClockPickerViewController { [weak self, weak popup] choice in
      guard let self, let popup else { return }
      self.selectedClockChoices[slot] = choice
      popup.item(at: 0)?.title = choice.title(at: Date())
      self.clockPickerPanel?.close()
      self.onClockChoicesChange(self.selectedClockChoices)
    }
    guard let window = popup.window else { return }
    let anchor = window.convertToScreen(popup.convert(popup.bounds, to: nil))
    let panel = ClockPickerPanel(
      contentRect: NSRect(
        x: anchor.minX,
        y: anchor.minY - 320,
        width: clockMenuWidth,
        height: 320
      ),
      styleMask: .borderless,
      backing: .buffered,
      defer: false
    )
    panel.isReleasedWhenClosed = false
    panel.hasShadow = true
    panel.backgroundColor = .windowBackgroundColor
    panel.contentViewController = picker
    panel.contentView?.wantsLayer = true
    panel.contentView?.layer?.cornerRadius = 8
    panel.contentView?.layer?.masksToBounds = true
    clockPickerPanel?.close()
    clockPickerPanel = panel
    panel.makeKeyAndOrderFront(nil)
  }

}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate,
  NSMenuDelegate, @preconcurrency LocalProcessTerminalViewDelegate
{
  private let clocksDefaultsKey = "selectedClockCities"
  private let legacyClocksDefaultsKey = "selectedClockTimezones"
  private let launchAgentLabel = "com.priyashpatil.stats"
  private let windowPlacementDefaultsKey = "mainWindowPlacement"
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
    restoreWindowPlacement(window)
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
    saveWindowPlacement()
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
    saveWindowPlacement()
  }

  func windowDidResize(_ notification: Notification) {
    saveWindowPlacement()
  }

  func windowDidChangeScreen(_ notification: Notification) {
    saveWindowPlacement()
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
      selectedClockChoices: selectedClockChoices,
      launchesAtLogin: launchesAtLogin,
      onLaunchAtLoginChange: { [weak self] enabled in
        self?.setLaunchAtLogin(enabled) ?? false
      },
      onClockChoicesChange: { [weak self] choices in
        guard let self else { return }
        UserDefaults.standard.set(choices.map(\.id), forKey: self.clocksDefaultsKey)
        self.restartTerminal()
      }
    )
    settingsWindowController = controller
    controller.showWindow(nil)
    controller.window?.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
  }

  private func processEnvironment(home: String) -> [String] {
    var environment = ProcessInfo.processInfo.environment
    environment.removeValue(forKey: "NO_COLOR")
    environment["HOME"] = home
    environment["TERM"] = "xterm-256color"
    environment["COLORTERM"] = "truecolor"
    let clocks = selectedClockChoices
    if let data = try? JSONEncoder().encode(clocks),
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

  private var selectedClockChoices: [ClockChoice] {
    let storedIDs = UserDefaults.standard.stringArray(forKey: clocksDefaultsKey)
    var choices =
      storedIDs?.compactMap { id in
        ClockChoice.all.first(where: { $0.id == id })
      } ?? []
    if storedIDs == nil {
      choices =
        UserDefaults.standard.stringArray(forKey: legacyClocksDefaultsKey)?
        .compactMap(ClockChoice.choice(forLegacyTimezone:)) ?? []
    }
    for choice in ClockChoice.defaults where choices.count < 4 {
      if !choices.contains(where: { $0.id == choice.id }) {
        choices.append(choice)
      }
    }
    return Array(choices.prefix(4))
  }

  private var launchesAtLogin: Bool {
    let process = Process()
    let output = Pipe()
    process.executableURL = URL(fileURLWithPath: "/bin/launchctl")
    process.arguments = ["print-disabled", "gui/\(getuid())"]
    process.standardOutput = output
    process.standardError = FileHandle.nullDevice
    do {
      try process.run()
      process.waitUntilExit()
    } catch {
      return true
    }
    let data = output.fileHandleForReading.readDataToEndOfFile()
    let disabledServices = String(data: data, encoding: .utf8) ?? ""
    return !disabledServices.contains("\"\(launchAgentLabel)\" => disabled")
  }

  private func setLaunchAtLogin(_ enabled: Bool) -> Bool {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/bin/launchctl")
    process.arguments = [
      enabled ? "enable" : "disable",
      "gui/\(getuid())/\(launchAgentLabel)",
    ]
    do {
      try process.run()
      process.waitUntilExit()
    } catch {
      NSSound.beep()
      return launchesAtLogin
    }
    if process.terminationStatus != 0 {
      NSSound.beep()
      return launchesAtLogin
    }
    return enabled
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

  private func restoreWindowPlacement(_ window: NSWindow) {
    guard let screen = NSScreen.screens.first else { return }
    let visibleFrame = screen.visibleFrame
    let placement = UserDefaults.standard.dictionary(forKey: windowPlacementDefaultsKey)
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

  private func saveWindowPlacement() {
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
    UserDefaults.standard.set(
      [
        "x": frame.minX - visibleFrame.minX,
        "top": visibleFrame.maxY - frame.maxY,
        "width": frame.width,
        "height": frame.height,
      ],
      forKey: windowPlacementDefaultsKey
    )
  }

  private func showMainWindow(_ window: NSWindow) {
    restoreWindowPlacement(window)
    window.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
    DispatchQueue.main.async { [weak self, weak window] in
      guard let self, let window else { return }
      self.restoreWindowPlacement(window)
      window.makeKeyAndOrderFront(nil)
      self.saveWindowPlacement()
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

@main
struct StatsApplication {
  @MainActor
  static func main() {
    let application = NSApplication.shared
    let delegate = AppDelegate()
    application.delegate = delegate
    application.setActivationPolicy(.accessory)
    application.run()
  }
}
