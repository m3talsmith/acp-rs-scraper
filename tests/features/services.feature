Feature: Services, the plugin pin, and quarantine
  The package gate runs against a file in a throwaway HOME. Nothing here
  contacts acp.us, x.com, or youtube.com.

  Scenario: Doctor prints the plugin pin and nothing else
    When I run "--doctor"
    Then it succeeds
    And stdout contains "rustc: rustc "
    And stdout contains "core: acp_red_star_node_core "
    And stdout does not contain "redstar"

  Scenario: Red Star is listed on and a bad package is quarantined
    Given a plugin archive "bad-linux-x86_64-0.1.0.tar.gz" that fails the package check
    When I run "--services"
    Then it succeeds
    And stdout contains "redstar  Red Star  enabled"
    And stdout does not contain "rustc:"
    When I run "--quarantine"
    Then it succeeds
    And stdout contains "bad-linux-x86_64-0.1.0.tar.gz"
    And stdout contains "potential security threat"
