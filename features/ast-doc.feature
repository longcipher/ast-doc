Feature: ast-doc llms.txt Generation
  As a developer
  I want to generate optimized llms.txt documentation from my codebase
  So that I can provide LLMs with an efficient token-optimized code overview

  Scenario: Generate llms.txt from a project
    Given a project directory with source files
    When I run ast-doc on the project directory
    Then the output should contain a "Repository" header
    And the output should contain a "Directory Tree" section
    And the output should contain a "Source Files" section
    And each source file should have a strategy annotation

  Scenario: Enforce token budget
    Given a project directory with source files totalling 10000 tokens
    When I run ast-doc with max-tokens set to 5000
    Then the output should not exceed 5000 tokens
    And files should be degraded to NoTests or Summary strategy

  Scenario: Protect core files from degradation
    Given a project directory with source files totalling 10000 tokens
    And a core file pattern matching "src/core/**"
    When I run ast-doc with max-tokens set to 5000
    Then files matching the core pattern should remain in Full strategy
    And non-core files should be degraded first

  Scenario: Strip test modules in NoTests mode
    Given a Rust source file containing a test module with "#[cfg(test)]"
    When I run ast-doc with strategy set to no-tests
    Then the test module should not appear in the output
    And the production code should be preserved
    And a marker indicating test removal should be present

  Scenario: Extract signatures in Summary mode
    Given a Rust source file containing functions with implementations
    When I run ast-doc with strategy set to summary
    Then only function signatures should appear in the output
    And function bodies should be replaced with an omission marker
    And docstrings should be preserved

  Scenario: Respect include and exclude patterns
    Given a project directory with .rs and .py and .txt files
    When I run ast-doc with include pattern "*.rs"
    Then only Rust files should appear in the output
    When I run ast-doc with exclude pattern "*.txt"
    Then text files should not appear in the output

  Scenario: Succeed with warning when budget is insufficient
    Given a project directory with source files totalling 50000 tokens
    And a git diff totalling 5000 tokens
    When I run ast-doc with max-tokens set to 1000
    Then ast-doc should succeed with a budget warning
