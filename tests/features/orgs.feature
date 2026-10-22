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

  Scenario: An invite adds a member only when the fingerprint matches
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-identity"
    Then it succeeds
    And stdout contains "fingerprint:"
    And stdout does not contain "sign_seed"
    And the file "$HOME/.acp/ai/data/org-identity.key" exists
    When I run "--org-invite hold --role Viewer --org-invite-out $HOME/invite.json --dry-run"
    Then it succeeds
    And stdout contains "dry-run: nothing was written"
    And the file "$HOME/invite.json" does not exist
    When I run "--org-invite hold --role Owner --org-invite-out $HOME/owner.json"
    Then it fails
    And stderr contains "Owner"
    And the file "$HOME/owner.json" does not exist
    When I run "--org-invite hold --role Viewer --org-invite-serve 0.0.0.0:9"
    Then it fails
    And stderr contains "127.0.0.1"
    When I run "--org-invite hold --role Viewer --org-invite-out $HOME/invite.json"
    Then it succeeds
    And stdout contains "invite written"
    And the file "$HOME/invite.json" exists
    And the file "$HOME/invite.json" contains "inviter_ed25519"
    And the file "$HOME/invite.json" does not contain "sign_seed"
    When I run in a second home "--org-invite-accept $HOME/invite.json --org-fingerprint 00000000 --org-invite-reply $HOME/reply.json"
    Then it fails
    And stdout contains "fingerprint:"
    And stderr contains "fingerprint does not match"
    And the file "$HOME/reply.json" does not exist
    When I run in a second home with that fingerprint "--org-invite-accept $HOME/invite.json --org-fingerprint $FINGERPRINT --org-invite-reply $HOME/reply.json"
    Then it succeeds
    And stdout contains "invite accepted"
    And the file "$HOME/reply.json" exists
    And the file "$HOME/reply.json" does not contain "sign_seed"
    When I run "--org-invite-finish $HOME/reply.json --org-fingerprint 00000000"
    Then it fails
    And stderr contains "fingerprint does not match"
    When I run "--org-members hold"
    Then it succeeds
    And stdout contains "Owner"
    And stdout does not contain "Viewer"
    When I run with that fingerprint "--org-invite-finish $HOME/reply.json --org-fingerprint $FINGERPRINT"
    Then it succeeds
    And stdout contains "member added"
    And stdout contains "role: Viewer"
    When I run "--org-members hold"
    Then it succeeds
    And stdout contains "Viewer"
    And stdout contains "active"
    When I run with that fingerprint "--org-invite-finish $HOME/reply.json --org-fingerprint $FINGERPRINT"
    Then it fails
    And stderr contains "already used"

  Scenario: Custom roles sit between Viewer and Owner and the last Owner stays
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-role-create hold Scribe --rank 100 --permissions view,post --dry-run"
    Then it succeeds
    And stdout contains "would create role Scribe"
    And stdout contains "dry-run: nothing was written"
    When I run "--org-roles hold"
    Then it succeeds
    And stdout does not contain "Scribe"
    When I run "--org-role-create hold Over --rank 1000 --permissions view"
    Then it fails
    And stderr contains "1 through 999"
    When I run "--org-role-create hold Viewer --rank 10 --permissions view"
    Then it fails
    And stderr contains "built-in"
    When I run "--org-role-create hold Scribe --rank 100 --permissions view,post"
    Then it succeeds
    And stdout contains "Scribe"
    And stdout contains "rank 100"
    When I run "--org-default-register-role hold User"
    Then it succeeds
    And stdout contains "register default: User"
    When I run "--org-default-invite-role hold Owner"
    Then it fails
    And stderr contains "Owner"
    When I run "--org-roles hold"
    Then it succeeds
    And stdout contains "Scribe"
    And stdout contains "register default: User"
    And stdout contains "invite default: Viewer"
    And stdout contains "last owner"
    When I run "--org-members hold"
    Then it succeeds
    When I run with that member "--org-member-remove hold $MEMBER --dry-run"
    Then it succeeds
    And stdout contains "would remove member"
    And stdout contains "their posts stay"
    And stdout contains "dry-run: nothing was written"
    When I run with that member "--org-member-remove hold $MEMBER"
    Then it fails
    And stderr contains "refused: stdin is not a terminal"
    When I run "--org-members hold"
    Then it succeeds
    And stdout contains "Owner"
    When I run with that member "--org-member-remove hold $MEMBER --yes"
    Then it fails
    And stderr contains "last owner"
    When I run "--org-post hold --org-title Notes --body local"
    Then it succeeds
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "id:"
    When I run with that post "--org-archive $POST"
    Then it fails
    And stderr contains "refused: stdin is not a terminal"
    When I run with that post "--org-archive $POST --dry-run"
    Then it succeeds
    And stdout contains "would archive"
    And stdout contains "kind: update"
    And stdout contains "not a delete"
    And stdout contains "dry-run: nothing was written"
    When I run "--org-show hold"
    Then it succeeds
    And stdout does not contain "archived: yes"
    When I run with that post "--org-archive $POST --yes"
    Then it succeeds
    And stdout contains "archived"
    And stdout contains "kind: update"
    And stdout contains "not a delete"
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "archived: yes"
    And stdout contains "Notes"

  Scenario: Gating asks first and a member export is not the open site
    When I run "--org-create hold"
    Then it succeeds
    When I run "--org-post hold --org-title Secret --body hidden-body"
    Then it succeeds
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "gated: yes"
    And stdout contains "registration: closed"
    And stdout contains "hidden-body"
    When I run "--org-snapshot hold"
    Then it succeeds
    And stdout does not contain "hidden-body"
    When I run "--org-ungate hold --dry-run"
    Then it succeeds
    And stdout contains "would start seeing"
    And stdout contains "dry-run: nothing was written"
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "gated: yes"
    When I run "--org-ungate hold"
    Then it fails
    And stderr contains "refused: stdin is not a terminal"
    When I run "--org-ungate hold --yes"
    Then it succeeds
    And stdout contains "gated: no"
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "gated: no"
    When I run "--org-snapshot hold"
    Then it succeeds
    And stdout does not contain "hidden-body"
    When I run "--org-gate hold --yes"
    Then it succeeds
    And stdout contains "gated: yes"
    When I run "--org-registration hold open"
    Then it succeeds
    And stdout contains "registration: open"
    When I run "--org-show hold"
    Then it succeeds
    And stdout contains "registration: open"
    And stdout contains "gated: yes"
    When I run "--org-registrations hold"
    Then it succeeds
    And stdout contains "No pending registrations"
    When I run "--org-edit hold --org-visibility public --org-default-visibility public --yes"
    Then it succeeds
    When I run "--org-post hold --org-title Visible --body member-body --org-post-visibility public --yes"
    Then it succeeds
    When I run "--org-snapshot hold"
    Then it succeeds
    And stdout does not contain "member-body"
    And stdout does not contain "Visible"
    When I run "--org-export hold $HOME/member --org-member-export --dry-run"
    Then it succeeds
    And stdout contains "member content"
    And stdout contains "must not be published"
    And stdout contains "index.html"
    And stdout contains "dry-run: nothing was written"
    And the file "$HOME/member/index.html" does not exist
    When I run "--org-export hold $HOME/open --dry-run"
    Then it succeeds
    And stdout contains "would export the public site"
    And stdout contains "dry-run: nothing was written"
    And stdout does not contain "member content"
    And the file "$HOME/open/index.html" does not exist
    When I run "--org-export hold $HOME/open --yes"
    Then it succeeds
    And the file "$HOME/open/index.html" exists
    And the file "$HOME/open/index.html" does not contain "member-body"
    And the file "$HOME/open/index.html" does not contain "Visible"
