import Foundation

struct ClockChoice: Codable {
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

  func title(at date: Date) -> String {
    let formatter = DateFormatter()
    formatter.locale = Locale.current
    formatter.dateFormat = "EEE h:mm a"
    formatter.timeZone = TimeZone(identifier: timezone)
    return "\(label) — \(formatter.string(from: date)) — \(timezone)"
  }
}
