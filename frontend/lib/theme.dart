import 'package:flutter/material.dart';

/// DESIGN.md §2 — colors. Softened revision:
/// warmer near-blacks, gentler layering, desaturated accent, radius everywhere.
class T {
  // Agent Bot Design System (design.md)
  static const bg = Color(0xFFF5F5F7); // clean cool off-white page background
  static const bgSidebar = Color(0xFFFFFFFF); // crisp white sidebar
  static const panel = Color(0xFFFFFFFF); // crisp white card surface
  static const panel2 = Color(0xFFF7F7FA); // interactive subtle surface
  static const input = Color(0xFFEEEEF2); // input surface
  static const hairline = Color(0xFFE8E8EC); // soft card border and subtle divider
  static const hairlineStrong = Color(0xFFD6D6DC);
  static const fg = Color(0xFF171719); // bold high-contrast text-primary
  static const fg2 = Color(0xFF585862); // readable text-secondary
  static const fg3 = Color(0xFF85858D); // muted text-secondary / captions
  static const fg4 = Color(0xFFA8A8AF); // disabled text-muted
  static const accent = Color(0xFF2788E8); // bright goal blue
  static const accentStrong = Color(0xFF1B6BC0);
  static const up = Color(0xFF32C66A); // positive green from design.md
  static const down = Color(0xFFE66B79); // negative rose/red from design.md
  static const warn = Color(0xFFF2A52B); // orange from design.md
  static const info = Color(0xFF2788E8); // blue
  static const gold = Color(0xFFF2A52B); // orange / gold

  // Geometry
  static const radius = 20.0; // card radius from design.md §14: 20px
  static const radiusSm = 10.0; // buttons / inputs / chips
  static const radiusXs = 6.0; // tiny accents

  static const mono = 'HackMono';
  static const sans = 'FiraSans';

  static Color networkColor(String network) {
    switch (network.toLowerCase()) {
      case 'solana':
        return const Color(0xFF9D5BE8); // purple from design.md
      case 'ethereum':
        return const Color(0xFF2788E8); // blue from design.md
      case 'base':
        return const Color(0xFF0052FF); // base blue
      case 'arbitrum':
        return const Color(0xFF28A0F0); // arbitrum cyan
      case 'bsc':
      case 'binance':
        return const Color(0xFFF2A52B); // orange / gold
      default:
        return const Color(0xFF85858D);
    }
  }
}

/// DESIGN.md §3 — typography helpers (softened weights/spacing).
class Type {
  static const labelCaps = TextStyle(
    fontFamily: T.mono,
    fontSize: 10.5,
    fontWeight: FontWeight.w600,
    letterSpacing: 0.16,
    color: T.fg3,
  );
  static const panelTitle = TextStyle(
    fontFamily: T.sans,
    fontSize: 13,
    fontWeight: FontWeight.w700,
    letterSpacing: 0.2,
    color: T.fg,
  );
  static const hero = TextStyle(
    fontFamily: T.mono,
    fontSize: 22,
    fontWeight: FontWeight.w700,
    color: T.fg,
  );
  static const data = TextStyle(
    fontFamily: T.mono,
    fontSize: 13,
    color: T.fg,
    fontFeatures: [FontFeature.tabularFigures()],
  );
  static const dataBold = TextStyle(
    fontFamily: T.mono,
    fontSize: 13,
    fontWeight: FontWeight.w700,
    color: T.fg,
    fontFeatures: [FontFeature.tabularFigures()],
  );
  static const dataDim = TextStyle(
    fontFamily: T.mono,
    fontSize: 13,
    color: T.fg3,
    fontFeatures: [FontFeature.tabularFigures()],
  );
  static const body = TextStyle(
    fontFamily: T.sans,
    fontSize: 13,
    color: T.fg2,
    height: 1.45,
    letterSpacing: 0.01,
  );
}

/// DESIGN.md §5 — status chip coloring.
class StatusTone {
  Color get color => switch (state.toLowerCase()) {
        'paper_trading' || 'running' || 'shadow' || 'qualified' || 'filled' || 'active' => T.up,
        'paused' || 'pending' || 'queued' || 'stale' || 'partial' || 'open' =>
          T.warn,
        'faulted' || 'disabled' || 'rejected' || 'cancelled' || 'canceled' ||
        'closed' || 'failed' =>
          T.down,
        'ready' => T.fg3,
        _ => T.info,
      };
  final String state;
  StatusTone(this.state);
}

/// Full app theme — Grok Bot telemetry design system.
ThemeData buildTheme() {
  final base = ThemeData(
    brightness: Brightness.light,
    useMaterial3: true,
    scaffoldBackgroundColor: T.bg,
    fontFamily: T.sans,
    colorScheme: const ColorScheme.light(
      primary: T.accent,
      secondary: T.warn,
      error: T.down,
      surface: T.panel,
      onSurface: T.fg,
      onPrimary: Colors.white,
      outline: T.hairline,
    ),
  );

  return base.copyWith(
    textTheme: base.textTheme.apply(
      bodyColor: T.fg,
      displayColor: T.fg,
    ),
    dividerTheme: const DividerThemeData(color: T.hairline, thickness: 1, space: 1),
    cardTheme: CardThemeData(
      color: T.panel,
      surfaceTintColor: Colors.transparent,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(T.radius),
      ),
      margin: EdgeInsets.zero,
    ),
    snackBarTheme: SnackBarThemeData(
      backgroundColor: T.panel2,
      contentTextStyle: Type.data,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(T.radiusSm)),
      behavior: SnackBarBehavior.floating,
    ),
    inputDecorationTheme: InputDecorationTheme(
      isDense: true,
      filled: true,
      fillColor: T.input,
      contentPadding: const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
      hintStyle: Type.dataDim,
      labelStyle: Type.labelCaps,
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(T.radiusSm),
        borderSide: const BorderSide(color: T.hairline),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(T.radiusSm),
        borderSide: const BorderSide(color: T.hairline),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(T.radiusSm),
        borderSide: const BorderSide(color: T.accent, width: 1.1),
      ),
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: T.accent,
        foregroundColor: Colors.white,
        elevation: 0,
        minimumSize: const Size(0, 32),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 4),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(T.radiusSm)),
        textStyle: Type.labelCaps.copyWith(color: Colors.white, letterSpacing: 0.12),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: T.fg,
        backgroundColor: T.panel2,
        side: const BorderSide(color: T.hairline),
        minimumSize: const Size(0, 32),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 4),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(T.radiusSm)),
        textStyle: Type.labelCaps.copyWith(letterSpacing: 0.12),
      ),
    ),
    textButtonTheme: TextButtonThemeData(
      style: TextButton.styleFrom(
        foregroundColor: T.accent,
        textStyle: Type.labelCaps,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(T.radiusSm)),
        minimumSize: const Size(0, 32),
        padding: const EdgeInsets.symmetric(horizontal: 10),
      ),
    ),
    scrollbarTheme: ScrollbarThemeData(
      thumbColor: WidgetStatePropertyAll(T.fg4),
      thumbVisibility: const WidgetStatePropertyAll(false),
      radius: const Radius.circular(4),
    ),
    tooltipTheme: TooltipThemeData(
      textStyle: Type.data.copyWith(fontSize: 11, color: T.fg),
      decoration: BoxDecoration(
        color: T.panel2,
        borderRadius: BorderRadius.circular(T.radiusSm),
        border: Border.all(color: T.hairline),
      ),
    ),
    dataTableTheme: DataTableThemeData(
      headingTextStyle: Type.labelCaps,
      dataTextStyle: Type.data,
      headingRowColor: const WidgetStatePropertyAll(T.bg),
      headingRowHeight: 30,
      dataRowMinHeight: 28,
      dataRowMaxHeight: 30,
      horizontalMargin: 10,
      dividerThickness: 0,
    ),
  );
}

/// Terminal button — compact, rounded, caps mono.
class TermButton extends StatelessWidget {
  const TermButton({
    super.key,
    required this.label,
    this.onPressed,
    this.color,
    this.icon,
    this.tooltip,
  });

  final String label;
  final VoidCallback? onPressed;
  final Color? color;
  final IconData? icon;
  final String? tooltip;

  @override
  Widget build(BuildContext context) {
    const fg = Colors.white;
    final style = ElevatedButton.styleFrom(
      backgroundColor: color ?? T.accent,
      foregroundColor: fg,
      elevation: 0,
      minimumSize: const Size(0, 30),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(T.radiusSm)),
      textStyle: Type.labelCaps.copyWith(color: fg, letterSpacing: 0.12),
      disabledBackgroundColor: T.fg4,
      disabledForegroundColor: T.fg3,
    );
    final btn = ElevatedButton.icon(
      style: style,
      onPressed: onPressed,
      icon: icon == null ? const SizedBox.shrink() : Icon(icon, size: 13),
      label: Padding(
        padding: const EdgeInsets.only(left: 3),
        child: Text(label),
      ),
    );
    if (tooltip == null) return btn;
    return Tooltip(message: tooltip!, child: btn);
  }
}