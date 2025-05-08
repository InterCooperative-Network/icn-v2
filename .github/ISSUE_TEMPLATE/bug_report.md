name: 🐛 Bug Report
description: Report incorrect behavior, crashes, or unexpected outcomes.
title: "[BUG] "
labels: [bug, needs-triage]
body:
  - type: textarea
    attributes:
      label: What happened?
      description: Clearly describe the bug. What did you expect to happen instead?
    validations:
      required: true

  - type: textarea
    attributes:
      label: Steps to reproduce
      description: If possible, provide a minimal reproduction case (CLI commands, inputs, environment).
    validations:
      required: true

  - type: input
    attributes:
      label: OS and toolchain
      description: What OS, Rust version, and ICN version or commit are you using?

  - type: textarea
    attributes:
      label: Logs or stack trace
      description: Paste any relevant output. Use code blocks for readability.

  - type: checkboxes
    attributes:
      label: Affected components
      options:
        - label: Runtime
        - label: CLI
        - label: Wallet
        - label: Mesh
        - label: Federation DAG
        - label: Identity / VC 