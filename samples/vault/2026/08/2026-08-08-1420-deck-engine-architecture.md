---
id: 01KZG915P0GCW8T7SWM4FV4DK7
title: Deck engine architecture
created: 2026-08-08T14:20:00+05:30
labels: [Decaframe, Software Architecture]
---

Rough architecture for the deck engine:

```mermaid
graph LR
  Content[Content blocks] --> Layout[Layout chooser]
  Layout --> Theme[Theme tokens]
  Theme --> Render[Renderer]
  Render --> Export[PDF / PPTX]
```

Layout should never need a coordinate from the author.
