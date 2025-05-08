name: ✨ Feature Request
description: Suggest a new feature or enhancement
title: "[FEAT] "
labels: [enhancement, needs-triage]
body:
  - type: textarea
    attributes:
      label: Problem / Motivation
      description: What is missing or frustrating about the current system?

  - type: textarea
    attributes:
      label: Proposed solution
      description: What would you like ICN to support? Be as specific as possible.

  - type: checkboxes
    attributes:
      label: Affects
      options:
        - label: Runtime
        - label: Wallet / FFI
        - label: Mesh
        - label: Federation join/sync
        - label: CLI
        - label: Developer docs / onboarding
        - label: Observability / metrics 