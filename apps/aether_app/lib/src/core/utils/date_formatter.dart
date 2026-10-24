/// Formats a [DateTime] into a human-readable relative date string in Portuguese.
///
/// Output rules:
/// - Same calendar day: "Hoje"
/// - Previous calendar day: "Ontem"
/// - 2 to 7 days ago: "Semana Passada"
/// - 8 to 29 days ago: "Há X semanas"
/// - 30 to 364 days ago: "Há X meses"
/// - 365+ days ago: "Há X anos"
String formatRelativeDatePtBr(DateTime dateTime, [DateTime? clock]) {
  final now = clock ?? DateTime.now();

  DateTime startOfDay(DateTime dt) => DateTime(dt.year, dt.month, dt.day);

  final nowDay = startOfDay(now);
  final targetDay = startOfDay(dateTime);
  final diffDays = nowDay.difference(targetDay).inDays;

  if (diffDays <= 0) {
    return 'Hoje';
  } else if (diffDays == 1) {
    return 'Ontem';
  } else if (diffDays <= 7) {
    return 'Semana Passada';
  } else if (diffDays < 30) {
    final weeks = (diffDays / 7).floor();
    return 'Há $weeks semanas';
  } else if (diffDays < 365) {
    final months = (diffDays / 30).floor();
    return months <= 1 ? 'Há 1 mês' : 'Há $months meses';
  } else {
    final years = (diffDays / 365).floor();
    return years <= 1 ? 'Há 1 ano' : 'Há $years anos';
  }
}

