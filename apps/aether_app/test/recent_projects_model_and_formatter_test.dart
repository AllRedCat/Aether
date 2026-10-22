import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:aether_app/src/core/models/recent_project.dart';
import 'package:aether_app/src/core/utils/date_formatter.dart';

void main() {
  group('RecentProject Model', () {
    late Directory tempDir;

    setUp(() {
      tempDir = Directory.systemTemp.createTempSync('aether_test_model_');
    });

    tearDown(() {
      if (tempDir.existsSync()) {
        tempDir.deleteSync(recursive: true);
      }
    });

    test('serialization roundtrip (toJson / fromJson)', () {
      final now = DateTime(2026, 10, 15, 12, 0, 0);
      final project = RecentProject(
        name: 'Casamento Praia',
        bundlePath: '/path/to/Casamento.aether',
        lastOpenedAt: now,
      );

      final json = project.toJson();
      expect(json['name'], 'Casamento Praia');
      expect(json['bundlePath'], '/path/to/Casamento.aether');
      expect(json['lastOpenedAt'], now.toIso8601String());

      final restored = RecentProject.fromJson(json);
      expect(restored, equals(project));
      expect(restored.name, 'Casamento Praia');
      expect(restored.bundlePath, '/path/to/Casamento.aether');
      expect(restored.lastOpenedAt, now);
    });

    test('existsOnDisk is true only when <bundlePath>/project.db exists', () {
      final bundleDir = Directory('${tempDir.path}/MeuVideo.aether');
      bundleDir.createSync(recursive: true);

      final project = RecentProject(
        name: 'Meu Video',
        bundlePath: bundleDir.path,
        lastOpenedAt: DateTime.now(),
      );

      expect(project.existsOnDisk, isFalse);

      final dbFile = File('${bundleDir.path}/project.db');
      dbFile.writeAsStringSync('dummy sqlite');

      expect(project.existsOnDisk, isTrue);
    });

    test('copyWith and value equality', () {
      final d1 = DateTime(2026, 1, 1);
      final p1 = RecentProject(name: 'A', bundlePath: '/a', lastOpenedAt: d1);
      final p2 = RecentProject(name: 'A', bundlePath: '/a', lastOpenedAt: d1);
      final p3 = p1.copyWith(name: 'B');

      expect(p1, equals(p2));
      expect(p1.hashCode, equals(p2.hashCode));
      expect(p3.name, 'B');
      expect(p3.bundlePath, '/a');
      expect(p1 == p3, isFalse);
    });
  });

  group('formatRelativeDatePtBr', () {
    final referenceClock = DateTime(2026, 10, 15, 14, 0, 0);

    test('returns "Hoje" for same calendar day and future dates', () {
      final sameDayMorning = DateTime(2026, 10, 15, 8, 30, 0);
      expect(formatRelativeDatePtBr(sameDayMorning, referenceClock), 'Hoje');

      final futureDate = DateTime(2026, 10, 16, 10, 0, 0);
      expect(formatRelativeDatePtBr(futureDate, referenceClock), 'Hoje');
    });

    test('returns "Ontem" for yesterday', () {
      final yesterday = DateTime(2026, 10, 14, 18, 0, 0);
      expect(formatRelativeDatePtBr(yesterday, referenceClock), 'Ontem');
    });

    test('returns "Semana Passada" for 2 to 7 days ago', () {
      final twoDaysAgo = DateTime(2026, 10, 13, 12, 0, 0);
      expect(formatRelativeDatePtBr(twoDaysAgo, referenceClock), 'Semana Passada');

      final sevenDaysAgo = DateTime(2026, 10, 8, 12, 0, 0);
      expect(formatRelativeDatePtBr(sevenDaysAgo, referenceClock), 'Semana Passada');
    });

    test('returns "Há X semanas" for 8 to 29 days ago', () {
      final eightDaysAgo = DateTime(2026, 10, 7, 12, 0, 0);
      expect(formatRelativeDatePtBr(eightDaysAgo, referenceClock), 'Há 1 semanas');

      final twoWeeksAgo = DateTime(2026, 10, 1, 12, 0, 0);
      expect(formatRelativeDatePtBr(twoWeeksAgo, referenceClock), 'Há 2 semanas');

      final threeWeeksAgo = DateTime(2026, 9, 24, 12, 0, 0);
      expect(formatRelativeDatePtBr(threeWeeksAgo, referenceClock), 'Há 3 semanas');
    });

    test('returns "Há X meses" for 30 to 364 days ago', () {
      final oneMonthAgo = DateTime(2026, 9, 10, 12, 0, 0);
      expect(formatRelativeDatePtBr(oneMonthAgo, referenceClock), 'Há 1 mês');

      final twoMonthsAgo = DateTime(2026, 8, 10, 12, 0, 0);
      expect(formatRelativeDatePtBr(twoMonthsAgo, referenceClock), 'Há 2 meses');
    });

    test('returns "Há X anos" for >= 365 days ago', () {
      final oneYearAgo = DateTime(2025, 9, 15, 12, 0, 0);
      expect(formatRelativeDatePtBr(oneYearAgo, referenceClock), 'Há 1 ano');

      final twoYearsAgo = DateTime(2024, 9, 15, 12, 0, 0);
      expect(formatRelativeDatePtBr(twoYearsAgo, referenceClock), 'Há 2 anos');
    });
  });
}

