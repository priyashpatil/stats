import AppKit

struct LaunchAtLoginController {
  private let serviceLabel = "com.priyashpatil.stats"

  var isEnabled: Bool {
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
    return !disabledServices.contains("\"\(serviceLabel)\" => disabled")
  }

  func setEnabled(_ enabled: Bool) -> Bool {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/bin/launchctl")
    process.arguments = [
      enabled ? "enable" : "disable",
      "gui/\(getuid())/\(serviceLabel)",
    ]
    do {
      try process.run()
      process.waitUntilExit()
    } catch {
      NSSound.beep()
      return isEnabled
    }
    if process.terminationStatus != 0 {
      NSSound.beep()
      return isEnabled
    }
    return enabled
  }
}
