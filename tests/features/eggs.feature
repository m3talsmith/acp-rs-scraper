@eggs
Feature: A few surprises
  These stay out of --help. The search phrase does nothing on the command line.

  Scenario: A hidden roar
    When I run "--roar"
    Then it succeeds
    And stdout contains "@hazaldin"
    And stdout contains "Haz al Din"

  Scenario: Help does not list the roar
    When I run "--help"
    Then it succeeds
    And stdout does not contain "--roar"

  Scenario: Colour when nothing asks for plain text
    Given the environment has "NO_COLOR" set to ""
    When I run "--roar"
    Then it succeeds
    And stdout contains "[31m"
    And stdout contains "@hazaldin"

  Scenario: Plain text when NO_COLOR is set
    Given the environment has "NO_COLOR" set to "1"
    When I run "--roar"
    Then it succeeds
    And stdout contains "@hazaldin"
    And stdout does not contain "[31m"

  Scenario: Plain text when color is never
    Given the environment has "NO_COLOR" set to ""
    When I run "--color never --roar"
    Then it succeeds
    And stdout contains "@hazaldin"
    And stdout does not contain "[31m"

  Scenario: Always keeps the colour when NO_COLOR is set
    Given the environment has "NO_COLOR" set to "1"
    When I run "--color always --roar"
    Then it succeeds
    And stdout contains "[31m"

  Scenario: A command-line search of the phrase is only a search
    When I run "--search" with "one fucking life"
    Then it succeeds
    And stdout does not contain "Be an explosive fucking nebula"
    And stdout does not contain "@hazaldin"
