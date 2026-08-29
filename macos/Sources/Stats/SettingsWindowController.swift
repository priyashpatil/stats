import AppKit

private struct SectionOption {
  let title: String
  let keyPath: WritableKeyPath<SectionDisplayConfig, Bool>
}

private struct SectionDescriptor {
  let title: String
  let help: String
  let enabledKeyPath: WritableKeyPath<SectionsConfig, Bool>
  let options: [SectionOption]
  let restoreDefaults: (inout SectionDisplayConfig) -> Void
}

private struct SectionControl {
  let button: NSButton
  let section: SectionDescriptor
  let optionKeyPath: WritableKeyPath<SectionDisplayConfig, Bool>?
}

@MainActor
final class SettingsWindowController: NSWindowController, NSTableViewDataSource,
  NSTableViewDelegate
{
  private let detailController = NSViewController()
  private var panes: [NSViewController] = []
  private let sidebarItems = [
    (title: "General", symbol: "gearshape"),
    (title: "Clocks", symbol: "clock"),
    (title: "System", symbol: "cpu"),
    (title: "AI", symbol: "sparkles"),
    (title: "Amp Activity", symbol: "chart.bar"),
    (title: "Codex Activity", symbol: "calendar"),
    (title: "About", symbol: "info.circle"),
  ]
  private let sidebarTable = NSTableView()
  private let clockMenuWidth: CGFloat = 360
  private var clockPickerPanel: ClockPickerPanel?
  private var selectedClockChoices: [ClockChoice]
  private var selectedSections: SectionsConfig
  private var selectedSectionDisplay: SectionDisplayConfig
  private var sectionControls: [ObjectIdentifier: SectionControl] = [:]
  private var clockPickers: [NSControl] = []
  private let clocksSection = SectionDescriptor(
    title: "Clocks",
    help: "Type a city or time zone in any clock field, then choose a matching city.",
    enabledKeyPath: \SectionsConfig.clocks,
    options: [
      SectionOption(title: "Section heading", keyPath: \SectionDisplayConfig.clocks.heading),
      SectionOption(title: "Clock 1", keyPath: \SectionDisplayConfig.clocks.clock1),
      SectionOption(title: "Clock 2", keyPath: \SectionDisplayConfig.clocks.clock2),
      SectionOption(title: "Clock 3", keyPath: \SectionDisplayConfig.clocks.clock3),
      SectionOption(title: "Clock 4", keyPath: \SectionDisplayConfig.clocks.clock4),
    ],
    restoreDefaults: { $0.clocks = ClocksDisplayConfig() }
  )
  private let systemSection = SectionDescriptor(
    title: "System",
    help: "Choose which system metrics Stats displays.",
    enabledKeyPath: \SectionsConfig.system,
    options: [
      SectionOption(title: "Section heading", keyPath: \SectionDisplayConfig.system.heading),
      SectionOption(title: "CPU", keyPath: \SectionDisplayConfig.system.cpu),
      SectionOption(title: "RAM", keyPath: \SectionDisplayConfig.system.ram),
      SectionOption(title: "GPU", keyPath: \SectionDisplayConfig.system.gpu),
      SectionOption(title: "Storage", keyPath: \SectionDisplayConfig.system.storage),
      SectionOption(title: "Network", keyPath: \SectionDisplayConfig.system.network),
    ],
    restoreDefaults: { $0.system = SystemDisplayConfig() }
  )
  private let aiSection = SectionDescriptor(
    title: "AI",
    help: "Choose which AI usage information Stats displays.",
    enabledKeyPath: \SectionsConfig.ai,
    options: [
      SectionOption(title: "Section heading", keyPath: \SectionDisplayConfig.ai.heading),
      SectionOption(title: "Amp plan usage", keyPath: \SectionDisplayConfig.ai.ampPlan),
      SectionOption(title: "Amp Orbs", keyPath: \SectionDisplayConfig.ai.ampOrbs),
      SectionOption(title: "Amp credits", keyPath: \SectionDisplayConfig.ai.ampCredits),
      SectionOption(title: "Codex quota", keyPath: \SectionDisplayConfig.ai.codexQuota),
      SectionOption(title: "Claude quota", keyPath: \SectionDisplayConfig.ai.claudeQuota),
      SectionOption(
        title: "Antigravity quota",
        keyPath: \SectionDisplayConfig.ai.antigravityQuota
      ),
      SectionOption(title: "Cursor quota", keyPath: \SectionDisplayConfig.ai.cursorQuota),
      SectionOption(title: "Grok quota", keyPath: \SectionDisplayConfig.ai.grokQuota),
    ],
    restoreDefaults: { $0.ai = AIDisplayConfig() }
  )
  private let ampActivitySection = SectionDescriptor(
    title: "Amp Activity",
    help: "Choose which Amp activity details Stats displays.",
    enabledKeyPath: \SectionsConfig.ampActivity,
    options: [
      SectionOption(title: "Section heading", keyPath: \SectionDisplayConfig.ampActivity.heading),
      SectionOption(
        title: "Activity calendar",
        keyPath: \SectionDisplayConfig.ampActivity.calendar
      ),
      SectionOption(
        title: "Daily activity",
        keyPath: \SectionDisplayConfig.ampActivity.dailyActivity
      ),
      SectionOption(
        title: "Cost and runtime summary",
        keyPath: \SectionDisplayConfig.ampActivity.usageSummary
      ),
      SectionOption(title: "Models", keyPath: \SectionDisplayConfig.ampActivity.models),
      SectionOption(title: "Sources", keyPath: \SectionDisplayConfig.ampActivity.sources),
      SectionOption(
        title: "Sync alerts",
        keyPath: \SectionDisplayConfig.ampActivity.syncAlerts
      ),
    ],
    restoreDefaults: { $0.ampActivity = AmpActivityDisplayConfig() }
  )
  private let codexActivitySection = SectionDescriptor(
    title: "Codex Activity",
    help: "Choose which Codex activity details Stats displays.",
    enabledKeyPath: \SectionsConfig.codexActivity,
    options: [
      SectionOption(
        title: "Section heading",
        keyPath: \SectionDisplayConfig.codexActivity.heading
      ),
      SectionOption(
        title: "Activity calendar",
        keyPath: \SectionDisplayConfig.codexActivity.calendar
      ),
      SectionOption(
        title: "Usage overview",
        keyPath: \SectionDisplayConfig.codexActivity.overview
      ),
      SectionOption(
        title: "Daily activity",
        keyPath: \SectionDisplayConfig.codexActivity.dailyActivity
      ),
    ],
    restoreDefaults: { $0.codexActivity = CodexActivityDisplayConfig() }
  )
  private let onLaunchAtLoginChange: (Bool) -> Bool
  private let onFontSizeChange: (Int) -> Void
  private let onShowScrollbarChange: (Bool) -> Bool
  private let onSectionSettingsChange: (SectionsConfig, SectionDisplayConfig) -> Bool
  private let onClockChoicesChange: ([ClockChoice]) -> Void
  private let onOpenConfig: () -> Void

  init(
    selectedClockChoices: [ClockChoice],
    launchesAtLogin: Bool,
    fontSize: Int,
    showsScrollbar: Bool,
    sections: SectionsConfig,
    sectionDisplay: SectionDisplayConfig,
    configPath: String,
    onLaunchAtLoginChange: @escaping (Bool) -> Bool,
    onFontSizeChange: @escaping (Int) -> Void,
    onShowScrollbarChange: @escaping (Bool) -> Bool,
    onSectionSettingsChange: @escaping (SectionsConfig, SectionDisplayConfig) -> Bool,
    onClockChoicesChange: @escaping ([ClockChoice]) -> Void,
    onOpenConfig: @escaping () -> Void
  ) {
    self.selectedClockChoices = selectedClockChoices
    self.selectedSections = sections
    self.selectedSectionDisplay = sectionDisplay
    self.onLaunchAtLoginChange = onLaunchAtLoginChange
    self.onFontSizeChange = onFontSizeChange
    self.onShowScrollbarChange = onShowScrollbarChange
    self.onSectionSettingsChange = onSectionSettingsChange
    self.onClockChoicesChange = onClockChoicesChange
    self.onOpenConfig = onOpenConfig
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
    configureContent(
      selectedClockChoices: selectedClockChoices,
      launchesAtLogin: launchesAtLogin,
      fontSize: fontSize,
      showsScrollbar: showsScrollbar,
      sections: sections,
      sectionDisplay: sectionDisplay,
      configPath: configPath
    )
  }

  required init?(coder: NSCoder) {
    fatalError("init(coder:) has not been implemented")
  }

  private func configureContent(
    selectedClockChoices: [ClockChoice],
    launchesAtLogin: Bool,
    fontSize: Int,
    showsScrollbar: Bool,
    sections: SectionsConfig,
    sectionDisplay: SectionDisplayConfig,
    configPath: String
  ) {
    panes = [
      generalViewController(
        launchesAtLogin: launchesAtLogin,
        fontSize: fontSize,
        showsScrollbar: showsScrollbar,
        configPath: configPath
      ),
      clocksViewController(
        selectedClockChoices: selectedClockChoices,
        section: clocksSection,
        sections: sections,
        display: sectionDisplay
      ),
      sectionViewController(
        systemSection,
        sections: sections,
        display: sectionDisplay
      ),
      sectionViewController(
        aiSection,
        sections: sections,
        display: sectionDisplay
      ),
      sectionViewController(
        ampActivitySection,
        sections: sections,
        display: sectionDisplay
      ),
      sectionViewController(
        codexActivitySection,
        sections: sections,
        display: sectionDisplay
      ),
      AboutViewController(),
    ]
    updateSectionControls()

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
    appName.font = .systemFont(ofSize: 17, weight: .semibold)
    let appHeader = NSStackView(views: [appIcon, appName])
    appHeader.orientation = .horizontal
    appHeader.alignment = .centerY
    appHeader.spacing = 10
    appHeader.translatesAutoresizingMaskIntoConstraints = false
    sidebar.addSubview(appHeader)

    let separator = NSBox()
    separator.boxType = .separator
    separator.translatesAutoresizingMaskIntoConstraints = false
    sidebar.addSubview(separator)

    let scrollView = NSScrollView()
    scrollView.drawsBackground = false
    scrollView.hasVerticalScroller = true
    scrollView.autohidesScrollers = true
    scrollView.documentView = sidebarTable
    scrollView.translatesAutoresizingMaskIntoConstraints = false
    sidebar.addSubview(scrollView)
    NSLayoutConstraint.activate([
      appHeader.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor, constant: 16),
      appHeader.topAnchor.constraint(equalTo: sidebar.topAnchor, constant: 16),
      appIcon.widthAnchor.constraint(equalToConstant: 44),
      appIcon.heightAnchor.constraint(equalToConstant: 44),
      separator.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor, constant: 12),
      separator.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor, constant: -12),
      separator.topAnchor.constraint(equalTo: appHeader.bottomAnchor, constant: 12),
      scrollView.leadingAnchor.constraint(equalTo: sidebar.leadingAnchor),
      scrollView.trailingAnchor.constraint(equalTo: sidebar.trailingAnchor),
      scrollView.topAnchor.constraint(equalTo: separator.bottomAnchor, constant: 8),
      scrollView.bottomAnchor.constraint(equalTo: sidebar.bottomAnchor),
    ])

    detailController.view = NSView()

    let splitViewController = NSSplitViewController()
    splitViewController.preferredContentSize = NSSize(width: 720, height: 440)
    splitViewController.splitView.isVertical = true
    splitViewController.splitView.dividerStyle = .thin

    let sidebarItem = NSSplitViewItem(sidebarWithViewController: sidebarController)
    sidebarItem.minimumThickness = 180
    sidebarItem.maximumThickness = 240
    sidebarItem.canCollapse = false
    splitViewController.addSplitViewItem(sidebarItem)

    let detailItem = NSSplitViewItem(viewController: detailController)
    detailItem.minimumThickness = 440
    splitViewController.addSplitViewItem(detailItem)

    window?.contentViewController = splitViewController
    window?.contentMinSize = NSSize(width: 660, height: 360)
    window?.setContentSize(splitViewController.preferredContentSize)

    sidebarTable.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false)
    showPane(at: 0)
  }

  private func sectionViewController(
    _ section: SectionDescriptor,
    sections: SectionsConfig,
    display: SectionDisplayConfig
  ) -> NSViewController {
    let controller = settingsPane(height: max(400, CGFloat(section.options.count * 28 + 220)))
    let title = NSTextField(labelWithString: section.title)
    title.font = .systemFont(ofSize: 22, weight: .semibold)
    let help = NSTextField(wrappingLabelWithString: section.help)
    help.textColor = .secondaryLabelColor
    let master = sectionCheckbox(
      title: "Show \(section.title)",
      section: section,
      selected: sections[keyPath: section.enabledKeyPath]
    )
    let displayHeading = NSTextField(labelWithString: "Display")
    displayHeading.font = .systemFont(ofSize: 13, weight: .semibold)
    let checkboxes = section.options.map { option in
      sectionCheckbox(
        title: option.title,
        section: section,
        optionKeyPath: option.keyPath,
        selected: display[keyPath: option.keyPath]
      )
    }
    let displayStack = NSStackView(views: [displayHeading] + checkboxes)
    displayStack.orientation = .vertical
    displayStack.alignment = .leading
    displayStack.spacing = 8

    let stack = NSStackView(views: [title, help, master, displayStack])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 10
    stack.setCustomSpacing(8, after: title)
    stack.setCustomSpacing(20, after: help)
    stack.setCustomSpacing(20, after: master)
    install(stack, in: controller.view)
    return controller
  }

  private func sectionCheckbox(
    title: String,
    section: SectionDescriptor,
    optionKeyPath: WritableKeyPath<SectionDisplayConfig, Bool>? = nil,
    selected: Bool
  ) -> NSButton {
    let checkbox = NSButton(
      checkboxWithTitle: title,
      target: self,
      action: #selector(sectionChanged(_:))
    )
    checkbox.state = selected ? .on : .off
    sectionControls[ObjectIdentifier(checkbox)] = SectionControl(
      button: checkbox,
      section: section,
      optionKeyPath: optionKeyPath
    )
    return checkbox
  }

  private func generalViewController(
    launchesAtLogin: Bool,
    fontSize: Int,
    showsScrollbar: Bool,
    configPath: String
  ) -> NSViewController {
    let controller = settingsPane(height: 500)
    let title = NSTextField(labelWithString: "General")
    title.font = .systemFont(ofSize: 22, weight: .semibold)
    let subtitle = NSTextField(
      wrappingLabelWithString: "Customize the terminal and choose how Stats starts on your Mac."
    )
    subtitle.textColor = .secondaryLabelColor

    let terminalHeading = NSTextField(labelWithString: "Terminal")
    terminalHeading.font = .systemFont(ofSize: 13, weight: .semibold)
    let fontSizeLabel = NSTextField(labelWithString: "Font size:")
    let fontSizePopup = NSPopUpButton(frame: .zero, pullsDown: false)
    for size in StatsConfigStore.availableFontSizes {
      fontSizePopup.addItem(withTitle: "\(size) pt")
      fontSizePopup.lastItem?.tag = size
    }
    fontSizePopup.selectItem(withTag: fontSize)
    fontSizePopup.target = self
    fontSizePopup.action = #selector(fontSizeChanged(_:))
    let fontSizeRow = NSStackView(views: [fontSizeLabel, fontSizePopup])
    fontSizeRow.orientation = .horizontal
    fontSizeRow.alignment = .centerY
    fontSizeRow.spacing = 12
    let fontSizeHelp = NSTextField(
      wrappingLabelWithString: "Adjust the text size used in the Stats terminal."
    )
    fontSizeHelp.textColor = .secondaryLabelColor
    let scrollbarCheckbox = NSButton(
      checkboxWithTitle: "Show dashboard scrollbar",
      target: self,
      action: #selector(showScrollbarChanged(_:))
    )
    scrollbarCheckbox.state = showsScrollbar ? .on : .off
    let terminalSection = NSStackView(
      views: [terminalHeading, fontSizeRow, fontSizeHelp, scrollbarCheckbox]
    )
    terminalSection.orientation = .vertical
    terminalSection.alignment = .leading
    terminalSection.spacing = 8

    let configHeading = NSTextField(labelWithString: "Configuration")
    configHeading.font = .systemFont(ofSize: 13, weight: .semibold)
    let configPathLabel = NSTextField(wrappingLabelWithString: configPath)
    configPathLabel.textColor = .secondaryLabelColor
    configPathLabel.lineBreakMode = .byTruncatingMiddle
    let openConfigButton = NSButton(
      title: "Open Configuration File…",
      target: self,
      action: #selector(openConfig(_:))
    )
    let configSection = NSStackView(views: [configHeading, configPathLabel, openConfigButton])
    configSection.orientation = .vertical
    configSection.alignment = .leading
    configSection.spacing = 8

    let separator = NSBox()
    separator.boxType = .separator

    let startupHeading = NSTextField(labelWithString: "Startup")
    startupHeading.font = .systemFont(ofSize: 13, weight: .semibold)
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
    let startupSection = NSStackView(views: [startupHeading, checkbox, help])
    startupSection.orientation = .vertical
    startupSection.alignment = .leading
    startupSection.spacing = 8

    let stack = NSStackView(
      views: [title, subtitle, terminalSection, configSection, separator, startupSection]
    )
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 8
    stack.setCustomSpacing(6, after: title)
    stack.setCustomSpacing(24, after: subtitle)
    stack.setCustomSpacing(20, after: terminalSection)
    stack.setCustomSpacing(20, after: configSection)
    stack.setCustomSpacing(20, after: separator)
    separator.widthAnchor.constraint(equalTo: stack.widthAnchor).isActive = true
    install(stack, in: controller.view)
    return controller
  }

  private func clocksViewController(
    selectedClockChoices: [ClockChoice],
    section: SectionDescriptor,
    sections: SectionsConfig,
    display: SectionDisplayConfig
  ) -> NSViewController {
    let controller = settingsPane(height: 620)

    let title = NSTextField(labelWithString: section.title)
    title.font = .systemFont(ofSize: 22, weight: .semibold)
    let help = NSTextField(wrappingLabelWithString: section.help)
    help.textColor = .secondaryLabelColor
    let master = sectionCheckbox(
      title: "Show \(section.title)",
      section: section,
      selected: sections[keyPath: section.enabledKeyPath]
    )
    let displayHeading = NSTextField(labelWithString: "Display")
    displayHeading.font = .systemFont(ofSize: 13, weight: .semibold)
    let displayCheckboxes = section.options.map { option in
      sectionCheckbox(
        title: option.title,
        section: section,
        optionKeyPath: option.keyPath,
        selected: display[keyPath: option.keyPath]
      )
    }
    let displayStack = NSStackView(views: [displayHeading] + displayCheckboxes)
    displayStack.orientation = .vertical
    displayStack.alignment = .leading
    displayStack.spacing = 8

    let clocksHeading = NSTextField(labelWithString: "Clock locations")
    clocksHeading.font = .systemFont(ofSize: 13, weight: .semibold)

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
      clockPickers.append(popup)
      rows.append([label, popup])
    }

    let grid = NSGridView(views: rows)
    grid.rowSpacing = 12
    grid.columnSpacing = 12
    grid.column(at: 0).xPlacement = .leading
    grid.column(at: 0).width = 72
    grid.column(at: 1).xPlacement = .fill
    grid.column(at: 1).width = 360

    let stack = NSStackView(views: [title, help, master, displayStack, clocksHeading, grid])
    stack.orientation = .vertical
    stack.alignment = .leading
    stack.spacing = 10
    stack.setCustomSpacing(8, after: title)
    stack.setCustomSpacing(20, after: help)
    stack.setCustomSpacing(20, after: master)
    stack.setCustomSpacing(20, after: displayStack)
    install(stack, in: controller.view)
    return controller
  }

  private func settingsPane(height: CGFloat = 400) -> NSViewController {
    let controller = NSViewController()
    controller.preferredContentSize = NSSize(width: 520, height: height)
    controller.view = NSView(frame: NSRect(origin: .zero, size: controller.preferredContentSize))
    return controller
  }

  private func install(_ stack: NSStackView, in view: NSView) {
    stack.translatesAutoresizingMaskIntoConstraints = false
    view.addSubview(stack)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 28),
      stack.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -28),
      stack.topAnchor.constraint(equalTo: view.topAnchor, constant: 52),
      stack.bottomAnchor.constraint(lessThanOrEqualTo: view.bottomAnchor, constant: -28),
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
    image.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 16, weight: .regular)
    let label = NSTextField(labelWithString: item.title)
    label.font = .systemFont(ofSize: 14, weight: .medium)
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
      image.widthAnchor.constraint(equalToConstant: 20),
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
    detailController.view.subviews.forEach { $0.removeFromSuperview() }

    let pane = panes[index]
    detailController.addChild(pane)

    let scrollView = NSScrollView()
    scrollView.drawsBackground = false
    scrollView.hasVerticalScroller = true
    scrollView.autohidesScrollers = true
    scrollView.translatesAutoresizingMaskIntoConstraints = false
    detailController.view.addSubview(scrollView)
    NSLayoutConstraint.activate([
      scrollView.leadingAnchor.constraint(equalTo: detailController.view.leadingAnchor),
      scrollView.trailingAnchor.constraint(equalTo: detailController.view.trailingAnchor),
      scrollView.topAnchor.constraint(equalTo: detailController.view.topAnchor),
      scrollView.bottomAnchor.constraint(equalTo: detailController.view.bottomAnchor),
    ])
    detailController.view.layoutSubtreeIfNeeded()

    let documentSize = NSSize(
      width: scrollView.contentSize.width,
      height: max(scrollView.contentSize.height, pane.preferredContentSize.height)
    )
    let documentView = FlippedView(frame: NSRect(origin: .zero, size: documentSize))
    documentView.autoresizingMask = [.width]
    pane.view.translatesAutoresizingMaskIntoConstraints = true
    pane.view.frame = documentView.bounds
    pane.view.autoresizingMask = [.width, .height]
    documentView.addSubview(pane.view)
    scrollView.documentView = documentView
  }

  @objc private func launchAtLoginChanged(_ sender: NSButton) {
    let requested = sender.state == .on
    sender.state = onLaunchAtLoginChange(requested) ? .on : .off
  }

  @objc private func fontSizeChanged(_ sender: NSPopUpButton) {
    onFontSizeChange(sender.selectedTag())
  }

  @objc private func showScrollbarChanged(_ sender: NSButton) {
    let requested = sender.state == .on
    if !onShowScrollbarChange(requested) {
      sender.state = requested ? .off : .on
    }
  }

  @objc private func sectionChanged(_ sender: NSButton) {
    guard let control = sectionControls[ObjectIdentifier(sender)] else { return }
    let enabled = sender.state == .on
    var sections = selectedSections
    var display = selectedSectionDisplay

    if let optionKeyPath = control.optionKeyPath {
      display[keyPath: optionKeyPath] = enabled
      if !control.section.options.contains(where: { display[keyPath: $0.keyPath] }) {
        sections[keyPath: control.section.enabledKeyPath] = false
      }
    } else {
      if enabled && !control.section.options.contains(where: { display[keyPath: $0.keyPath] }) {
        control.section.restoreDefaults(&display)
      }
      sections[keyPath: control.section.enabledKeyPath] = enabled
    }

    if onSectionSettingsChange(sections, display) {
      selectedSections = sections
      selectedSectionDisplay = display
    }
    updateSectionControls()
  }

  private func updateSectionControls() {
    for control in sectionControls.values {
      let masterEnabled = selectedSections[keyPath: control.section.enabledKeyPath]
      if let optionKeyPath = control.optionKeyPath {
        control.button.state = selectedSectionDisplay[keyPath: optionKeyPath] ? .on : .off
        control.button.isEnabled = masterEnabled
      } else {
        control.button.state = masterEnabled ? .on : .off
      }
    }
    let clockOptions = clocksSection.options.dropFirst().map {
      selectedSectionDisplay[keyPath: $0.keyPath]
    }
    for (picker, optionEnabled) in zip(clockPickers, clockOptions) {
      picker.isEnabled = selectedSections.clocks && optionEnabled
    }
  }

  @objc private func openConfig(_ sender: Any?) {
    onOpenConfig()
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

private final class FlippedView: NSView {
  override var isFlipped: Bool { true }
}
