name: 🧠 RFC Proposal
description: Submit a major design proposal (new governance flow, economic model, etc.)
title: "[RFC] "
labels: [rfc, design]
body:
  - type: textarea
    attributes:
      label: Summary
      description: One-paragraph summary of what you're proposing.

  - type: textarea
    attributes:
      label: Motivation
      description: Why does this matter? What problem does it solve or improve?

  - type: textarea
    attributes:
      label: Design overview
      description: How would this work technically? Provide diagrams, pseudocode, or key API changes if needed.

  - type: textarea
    attributes:
      label: Tradeoffs / alternatives
      description: Are there downsides? Did you consider other designs?

  - type: textarea
    attributes:
      label: Implementation plan
      description: If accepted, how should we phase this in?

  - type: checkboxes
    attributes:
      label: RFC stage
      options:
        - label: "💬 Draft for feedback"
        - label: "✅ Ready for approval"
        - label: "🚧 Implementation in progress" 