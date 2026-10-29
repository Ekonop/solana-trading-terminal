import 'dart:math';

import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';

import '../models/models.dart';
import '../theme.dart';

/// Equity curve with a soft drawdown tint — DESIGN.md §5 charts.
class EquityChart extends StatelessWidget {
  const EquityChart({super.key, required this.series, this.height = 220});

  final List<EquitySnapshot> series;
  final double height;

  @override
  Widget build(BuildContext context) {
    if (series.length < 2) {
      return SizedBox(
        height: height,
        child: Center(child: Text('EQUITY SERIES COLLECTING…', style: Type.labelCaps)),
      );
    }
    // Trim to the last 300 points for drawing sanity.
    final pts = series.length > 300 ? series.sublist(series.length - 300) : series;

    final minTs = pts.first.ts.toDouble();
    final maxTs = pts.last.ts.toDouble();
    final span = max(maxTs - minTs, 1.0);

    final vals = pts.map((e) => e.equityD).toList();
    final minV = vals.reduce(min) * 0.999;
    final maxV = vals.reduce(max) * 1.001;

    final spots = <FlSpot>[
      for (var i = 0; i < pts.length; i++)
        FlSpot((pts[i].ts - minTs) / span, pts[i].equityD),
    ];

    // Drawdown zone: points below their running peak get a soft down tint.
    var peak = double.negativeInfinity;
    final cur = <FlSpot>[];
    for (var i = 0; i < pts.length; i++) {
      final v = vals[i];
      peak = max(peak, v);
      final depth = (v - peak) / max(peak.abs(), 1e-9); // negative in dd
      if (depth < -0.005) {
        cur.add(spots[i]);
      } else {
        cur.clear();
      }
    }

    final tooltip = LineTouchTooltipData(
      getTooltipColor: (_) => T.panel2,
      tooltipBorderRadius: BorderRadius.circular(T.radiusSm),
      fitInsideHorizontally: true,
      fitInsideVertically: true,
      getTooltipItems: (touched) => [
        for (final t in touched)
          LineTooltipItem(
            '\$${t.y.toStringAsFixed(2)}',
            Type.data.copyWith(color: T.accent, fontWeight: FontWeight.w700),
          ),
      ],
    );

    return SizedBox(
      height: height,
      child: Stack(
        children: [
          if (cur.length >= 2)
            Positioned.fill(
              child: Padding(
                padding: const EdgeInsets.only(top: 8, bottom: 8),
                child: LineChart(
                  LineChartData(
                    minY: minV,
                    maxY: maxV,
                    titlesData: _axisTitles(),
                    gridData: _grid(minV, maxV),
                    borderData: FlBorderData(show: false),
                    lineTouchData: LineTouchData(touchTooltipData: tooltip),
                    lineBarsData: [
                      LineChartBarData(
                        spots: cur,
                        isCurved: true,
                        curveSmoothness: 0.35,
                        color: T.down.withValues(alpha: 0.18),
                        barWidth: 1.2,
                        dotData: const FlDotData(show: false),
                        belowBarData: BarAreaData(
                          show: true,
                          gradient: LinearGradient(
                            begin: Alignment.topCenter,
                            end: Alignment.bottomCenter,
                            colors: [
                              T.down.withValues(alpha: 0.10),
                              T.down.withValues(alpha: 0.02),
                            ],
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          Positioned.fill(
            child: Padding(
              padding: const EdgeInsets.only(top: 8, bottom: 8),
              child: LineChart(
                LineChartData(
                  minY: minV,
                  maxY: maxV,
                  titlesData: _axisTitles(),
                  gridData: const FlGridData(show: false),
                  borderData: FlBorderData(show: false),
                  lineTouchData: LineTouchData(touchTooltipData: tooltip),
                  lineBarsData: [
                    LineChartBarData(
                      spots: spots,
                      isCurved: true,
                      curveSmoothness: 0.35,
                      color: T.up,
                      barWidth: 2.2,
                      dotData: const FlDotData(show: false),
                      shadow: Shadow(
                        color: T.up.withValues(alpha: 0.30),
                        blurRadius: 6,
                        offset: const Offset(0, 2),
                      ),
                      belowBarData: BarAreaData(
                        show: true,
                        gradient: LinearGradient(
                          begin: Alignment.topCenter,
                          end: Alignment.bottomCenter,
                          colors: [
                            T.up.withValues(alpha: 0.18),
                            T.up.withValues(alpha: 0.01),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }

  /// Quiet horizontal-only grid: barely-there lines, no verticals.
  FlGridData _grid(double minV, double maxV) {
    return FlGridData(
      show: true,
      drawVerticalLine: false,
      horizontalInterval: (maxV - minV) / 4,
      getDrawingHorizontalLine: (_) =>
          FlLine(color: T.hairline.withValues(alpha: 0.35), strokeWidth: 0.5),
    );
  }

  FlTitlesData _axisTitles() {
    return const FlTitlesData(
      leftTitles: AxisTitles(sideTitles: SideTitles(showTitles: false)),
      rightTitles: AxisTitles(sideTitles: SideTitles(showTitles: false)),
      topTitles: AxisTitles(sideTitles: SideTitles(showTitles: false)),
      bottomTitles: AxisTitles(sideTitles: SideTitles(showTitles: false)),
    );
  }
}