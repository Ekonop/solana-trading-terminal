import 'package:flutter_test/flutter_test.dart';
import 'package:stt_desktop/models/models.dart';

void main() {
  group('JsonHelpers', () {
    test('accepts kebab-case keys', () {
      final m = {
        'volume-24h-usd': '1234.5',
        'drawdown-pct': '0.0012',
        'sample-points': 7,
      };
      expect(JsonHelpers.s(m, ['volume-24h-usd', 'volume_24h_usd']), '1234.5');
      expect(JsonHelpers.s(m, ['volume_24h_usd', 'volume-24h-usd']), '1234.5');
      expect(JsonHelpers.d(m, ['drawdown-pct']), closeTo(0.0012, 1e-12));
      expect(JsonHelpers.i(m, ['sample-points']), 7);
    });

    test('accepts snake_case keys', () {
      final m = {
        'volume_24h_usd': '99.9',
        'peak_equity_usd': '1500',
        'is_paper': 'true',
      };
      expect(JsonHelpers.s(m, ['volume-24h-usd', 'volume_24h_usd']), '99.9');
      expect(JsonHelpers.s(m, ['peak-equity-usd', 'peak_equity_usd']), '1500');
      expect(JsonHelpers.b(m, ['is-paper', 'is_paper']), isTrue);
    });

    test('falls back to defaults on missing keys', () {
      expect(JsonHelpers.s(const {}, ['nope'], 'fallback'), 'fallback');
      expect(JsonHelpers.s(const {}, ['nope']), '');
      expect(JsonHelpers.i(const {}, ['nope'], 42), 42);
      expect(JsonHelpers.d(const {}, ['nope']), 0);
      expect(JsonHelpers.b(const {}, ['nope'], true), isTrue);
      expect(JsonHelpers.sN(const {}, ['nope']), isNull);
    });

    test('parses string numbers and bools from strings', () {
      expect(JsonHelpers.i({'v': '128'}, ['v']), 128);
      expect(JsonHelpers.d({'v': '3.25'}, ['v']), closeTo(3.25, 1e-9));
      expect(JsonHelpers.b({'v': 'false'}, ['v']), isFalse);
      expect(JsonHelpers.b({'v': 'TRUE'}, ['v']), isTrue);
    });
  });

  group('MarketQuote.fromJson', () {
    test('parses snake_case live payload', () {
      final q = MarketQuote.fromJson({
        'mint': 'so11111111111111111111111111111111111111112',
        'symbol': 'SOL',
        'name': 'Solana',
        'price': '149.065002416220885',
        'volume_24h_usd': '1234567.89',
        'liquidity_usd': '500000.10',
        'stale': false,
        'observed_at': 1793226902086041,
        'quality': 0,
        'source': 'dexscreener',
      });
      expect(q.mint, 'so11111111111111111111111111111111111111112');
      expect(q.symbol, 'SOL');
      expect(q.price, '149.065002416220885');
      expect(q.volume24hUsd, '1234567.89');
      expect(q.liquidityUsd, '500000.10');
      expect(q.stale, isFalse);
      expect(q.observedAt, 1793226902086041);
      expect(q.quality, 0);
      expect(q.source, 'dexscreener');
    });

    test('parses kebab_case payload', () {
      final q = MarketQuote.fromJson({
        'mint': 'm',
        'symbol': 'BONK',
        'name': 'Bonk',
        'price': '0.0000023',
        'volume-24h-usd': '88',
        'liquidity-usd': '12',
        'stale': true,
        'observed-at': 1,
        'quality': 2,
        'source': 'mock',
      });
      expect(q.volume24hUsd, '88');
      expect(q.liquidityUsd, '12');
      expect(q.stale, isTrue);
      expect(q.quality, 2);
    });
  });

  group('DashboardStats.fromJson', () {
    test('parses and exposes numeric getters', () {
      final s = DashboardStats.fromJson({
        'current-equity-usd': '1500.25',
        'peak-equity-usd': '1510.00',
        'net-return-pct': '0.0028',
        'day-return-pct': '-0.0010',
        'drawdown-pct': '0.0018',
        'positions-value-usd': '75.12',
        'sample-points': 9,
      });
      expect(s.currentEquityUsd, '1500.25');
      expect(s.netReturnPctD, closeTo(0.0028, 1e-12));
      expect(s.dayReturnPctD, closeTo(-0.001, 1e-12));
      expect(s.drawdownPctD, closeTo(0.0018, 1e-12));
      expect(s.positionsValueD, closeTo(75.12, 1e-9));
      expect(s.samplePoints, 9);
    });
  });
}