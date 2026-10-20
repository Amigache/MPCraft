---
name: New UI Screen
description: Use when adding a new screen, panel, or view to the MPCraft React frontend — covers routing, layout, store wiring, loading and error states, and accessibility.
metadata:
  opencode/autoinvoke: true
---

# Adding a screen or panel to MPCraft

## Decide first: screen, panel, or tab

| Container | Use for | Example |
| --- | --- | --- |
| **Screen** (routed) | A top-level mode of work | Flight Data, Mission Planner, Configuration, Logs |
| **Panel** | A region within a screen | Map, HUD, message console, telemetry readouts |
| **Tab** | Sibling views within a panel | Quick / Actions / Messages / Status |

Do not create a routed screen for something that is one region of an existing screen. Screen
transitions are expensive; the user loses their context.

## Structure

```
apps/ui/src/
├── app/
│   ├── routes.tsx              # route table
│   └── layout/
│       ├── AppShell.tsx        # chrome: menu, status bar, connection indicator
│       └── StatusBar.tsx
├── screens/
│   ├── flight-data/
│   │   ├── index.tsx
│   │   ├── FlightDataScreen.tsx
│   │   ├── panels/
│   │   │   ├── MapPanel.tsx
│   │   │   ├── HudPanel.tsx
│   │   │   ├── TelemetryPanel.tsx
│   │   │   └── MessageConsole.tsx
│   │   └── store.ts            # screen-local selectors over the global store
│   └── mission-planner/
└── components/                 # shared, generic, reusable
```

**Rule:** a component in `components/` must not know which screen uses it. If it does, it belongs
in the screen folder.

## Wiring to data

1. **Never fetch in a component body for live data.** Subscribe to the store.
2. **Use selectors, never whole-store subscriptions.** A telemetry tick must not re-render a panel
   that does not display telemetry.
3. **Loading and error states are mandatory.** Every async region gets both.
4. **Empty states are designed, not accidental.** "No vehicle connected" is a first-class screen
   state with a call to action.

## Destructive actions

Arm, disarm, mission write, parameter write, and firmware flash require an explicit confirmation
dialog. The confirm button states the concrete consequence:

> Arm **"Ares"**? The propellers will spin. Make sure the area is clear.

Never make the destructive action the default focus. Never auto-dismiss.

## Accessibility

- Every interactive element is reachable by keyboard, with a visible focus ring.
- Form controls have associated `<label>` elements.
- Colour is never the only signal — severity badges carry text or an icon too.
- Charts have a text summary or an accessible data table fallback.
- Contrast ratio meets WCAG AA. The app is used in sunlight.

## Styling

- Use design tokens (`var(--color-*)`, `var(--space-*)`). Never a raw hex value in a component.
- Dark and light themes are both supported from the start — test both.
- Layout with CSS Grid and Flexbox. Avoid absolute positioning except for map overlays.

## Testing

- Vitest for the screen's store selectors and any formatting logic.
- Playwright for the critical flow that reaches this screen.

## Checklist

- [ ] Route registered (screen) or panel composed (panel)
- [ ] Store subscription uses a narrow selector
- [ ] Loading, error, and empty states implemented
- [ ] Destructive actions confirmed with a specific consequence message
- [ ] Keyboard navigable; focus visible; labels associated
- [ ] Verified in dark and light themes
- [ ] Contrast checked
- [ ] `npm run typecheck` and `npm run lint` pass
