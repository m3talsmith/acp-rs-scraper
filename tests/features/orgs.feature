Feature: Organizations
  A local organization is a source. It does not need a service.
  A new organization and a new post stay private.

  Background:
    Given a mock Red Star site

  Scenario: Create, post, and see the organization as a source
    When I run "--org-create hold --org-name Hold"
    Then it succeeds
    And stderr contains "organization hold created"
    And stdout contains "hold  visibility: private  default post visibility: private"
    And stdout contains "gated: yes"
    And stdout contains "registration: closed"
    When I run "--org-roles hold"
    Then it succeeds
    And stdout contains "Owner"
    And stdout contains "User"
    And stdout contains "Viewer"
    When I run "--org-members hold"
    Then it succeeds
    And stdout contains "Owner"
    And stdout contains "active"
    When I run "--org-list"
    Then it succeeds
    And stdout contains "hold"
    And stdout contains "private"
    When I run "--org-post hold --org-title Notes --body local"
    Then it succeeds
    And stdout contains "post visibility: private"
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "visibility: private"
    And stdout contains "default post visibility: private"
    And stdout contains "Notes"
    And stdout contains "author: hold"
    And stdout contains "post visibility: private"
    When I run "--list-nicks"
    Then it succeeds
    And stdout contains "hold"

  Scenario: Edit the name
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-edit hold --org-name Renamed"
    Then it succeeds
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "Renamed"

  Scenario: Delete asks, and a dry-run deletes nothing
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-delete hold --dry-run"
    Then it succeeds
    And stdout contains "would delete organization hold"
    And stdout contains "dry-run: nothing was written"
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "hold"
    When I run "--org-delete hold --yes"
    Then it succeeds
    And stdout contains "deleted hold"
    When I run "--org-list"
    Then it succeeds
    And stdout contains "No organizations yet"

  Scenario: A new organization and a new post stay private
    When I run "--org-create hold"
    Then it succeeds
    And stdout contains "visibility: private"
    And stdout contains "default post visibility: private"
    When I run "--org-post hold --org-title Notes --body local"
    Then it succeeds
    And stdout contains "post visibility: private"
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "post visibility: private"

  Scenario: Making an organization public without a terminal is refused
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-edit hold --org-visibility public"
    Then it fails
    And stderr contains "refused: stdin is not a terminal"
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "visibility: private"

  Scenario: A dry-run does not make an organization public
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-edit hold --org-visibility public --dry-run"
    Then it succeeds
    And stdout contains "would edit organization"
    And stdout contains "would make public content readable"
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "visibility: private"

  Scenario: The public snapshot leaves private posts out and a new default does not rewrite them
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-post hold --org-title Secret --body hidden-body"
    Then it succeeds
    When I run "--org-snapshot hold"
    Then it succeeds
    And stdout contains '"posts": []'
    And stdout does not contain "Secret"
    And stdout does not contain "hidden-body"
    When I run "--org-post hold --org-title Marked --body marked-body --org-post-visibility public --yes"
    Then it succeeds
    When I run "--org-snapshot hold"
    Then it succeeds
    And stdout does not contain "Marked"
    And stdout does not contain "marked-body"
    When I run "--org-edit hold --org-default-visibility public --yes"
    Then it succeeds
    When I run "--org-post hold --org-title Later --body after-switch"
    Then it succeeds
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "Later"
    And stdout contains "post visibility: public"
    And stdout contains "Secret"
    And stdout contains "post visibility: private"
    When I run "--org-snapshot hold"
    Then it succeeds
    And stdout does not contain "Later"
    And stdout does not contain "Secret"

  Scenario: The default template names Cinzel and Lora and a replacement shows up
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-template-show hold"
    Then it succeeds
    And stdout contains "Cinzel"
    And stdout contains "Lora"
    And stdout contains "serif"
    And stdout does not contain "Research starts here."
    When I write "<p>marker-line</p>" to "tmpl.html"
    When I run "--org-template-set hold --org-template-file $HOME/tmpl.html --dry-run"
    Then it succeeds
    And stdout contains "would replace the template"
    And stdout contains "dry-run: nothing was written"
    When I run "--org-template-show hold"
    Then it succeeds
    And stdout contains "Cinzel"
    When I run "--org-template-set hold --org-template-file $HOME/tmpl.html --yes"
    Then it succeeds
    And stdout contains "template for hold replaced"
    When I run "--org-template-show hold"
    Then it succeeds
    And stdout contains "marker-line"
    And stdout does not contain "Cinzel"
    When I run "--org-template-reset hold --yes"
    Then it succeeds
    When I run "--org-template-show hold"
    Then it succeeds
    And stdout contains "Cinzel"

  Scenario: A new file starts private and stays out of the public snapshot
    When I write "hello-bytes" to "banner.txt"
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-asset-add hold --file $HOME/banner.txt --dry-run"
    Then it succeeds
    And stdout contains "would copy"
    And stdout contains "dry-run: nothing was written"
    When I run "--org-asset-list hold"
    Then it succeeds
    And stdout contains "No files yet"
    When I run "--org-asset-add hold --file $HOME/banner.txt --yes"
    Then it succeeds
    And stdout contains "banner.txt"
    And stdout contains "visibility: private"
    When I run "--org-asset-list hold"
    Then it succeeds
    And stdout contains "banner.txt"
    And stdout contains "private"
    When I run "--org-snapshot hold"
    Then it succeeds
    And stdout does not contain "banner.txt"
    When I run "--org-asset-remove orga-missing --dry-run"
    Then it succeeds
    And stdout contains "would archive"
    And stdout contains "not a delete"

  Scenario: Export writes a public site and leaves a private post out
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-post hold --org-title Secret --body hidden-body"
    Then it succeeds
    When I run "--org-export hold $HOME/site --dry-run"
    Then it succeeds
    And stdout contains "would export"
    And stdout contains "dry-run: nothing was written"
    And the file "$HOME/site/index.html" does not exist
    When I run "--org-export hold $HOME/site --yes"
    Then it succeeds
    And stdout contains "left out"
    And stdout contains "3.49.0"
    And stdout does not contain "prepared, gated:"
    And the file "$HOME/site/index.html" exists
    And the file "$HOME/site/index.html" does not contain "hidden-body"
    And the file "$HOME/site/feed.json" exists
    And the file "$HOME/site/feed.json" does not contain "hidden-body"

  Scenario: Publishing is off until it is confirmed
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-publish hold --dry-run"
    Then it succeeds
    And stdout contains "would publish"
    And stdout contains "org-hold"
    And stdout contains "exposes nothing"
    And stdout contains "dry-run: nothing was written"
    And the file "$HOME/.acp/services/org-hold.toml" does not exist
    When I run "--org-publish hold"
    Then it fails
    And stderr contains "refused: stdin is not a terminal"
    When I run "--org-services"
    Then it succeeds
    And stdout contains "No organization services yet"
