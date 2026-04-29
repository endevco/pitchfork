#!/usr/bin/env bats

setup() {
  load test_helper/common_setup
  _common_setup
}

teardown() {
  _common_teardown
}

# ---------------------------------------------------------------------------
# Helper: wait up to 5s for a file to exist
# ---------------------------------------------------------------------------
wait_for_file() {
  local file="$1"
  for _ in $(seq 1 50); do
    if [[ -e "$file" ]]; then
      return 0
    fi
    sleep 0.1
  done
  echo "Timed out waiting for file: $file" >&2
  return 1
}

# ---------------------------------------------------------------------------
# on_output – filter (substring match)
# ---------------------------------------------------------------------------

@test "on_output with filter fires hook when output contains substring" {
  local marker="$TEST_TEMP_DIR/on_output_fired"

  create_pitchfork_toml <<EOF
[daemons.printer]
run = "bash -c 'sleep 0.2; echo hello world; sleep 60'"

[daemons.printer.hooks]
on_output = { filter = "hello", run = "touch $marker" }
EOF

  pitchfork supervisor start
  pitchfork start printer

  wait_for_file "$marker"
  assert_file_exists "$marker"

  pitchfork stop printer
}

@test "on_output with filter does not fire when output does not match" {
  local marker="$TEST_TEMP_DIR/on_output_fired"

  create_pitchfork_toml <<EOF
[daemons.printer]
run = "bash -c 'echo goodbye; sleep 60'"

[daemons.printer.hooks]
on_output = { filter = "hello", run = "touch $marker" }
EOF

  pitchfork supervisor start
  pitchfork start printer

  sleep 1
  assert_file_not_exists "$marker"

  pitchfork stop printer
}

# ---------------------------------------------------------------------------
# on_output – regex match
# ---------------------------------------------------------------------------

@test "on_output with regex fires hook on matching line" {
  local marker="$TEST_TEMP_DIR/on_output_regex"

  create_pitchfork_toml <<EOF
[daemons.printer]
run = "bash -c 'sleep 0.2; echo port 3000; sleep 60'"

[daemons.printer.hooks]
on_output = { regex = "port [0-9]+", run = "touch $marker" }
EOF

  pitchfork supervisor start
  pitchfork start printer

  wait_for_file "$marker"
  assert_file_exists "$marker"

  pitchfork stop printer
}

@test "on_output with regex does not fire when line does not match" {
  local marker="$TEST_TEMP_DIR/on_output_regex"

  create_pitchfork_toml <<EOF
[daemons.printer]
run = "bash -c 'echo no numbers here; sleep 60'"

[daemons.printer.hooks]
on_output = { regex = "port [0-9]+", run = "touch $marker" }
EOF

  pitchfork supervisor start
  pitchfork start printer

  sleep 1
  assert_file_not_exists "$marker"

  pitchfork stop printer
}

# ---------------------------------------------------------------------------
# on_output – no filter/regex (fires on every line, subject to debounce)
# ---------------------------------------------------------------------------

@test "on_output without filter or regex fires on any output line" {
  local counter="$TEST_TEMP_DIR/counter"

  create_pitchfork_toml <<EOF
[daemons.printer]
run = "bash -c 'sleep 0.2; echo line1; sleep 60'"

[daemons.printer.hooks]
on_output = { run = "sh -c 'echo x >> $counter'" }
EOF

  pitchfork supervisor start
  pitchfork start printer

  wait_for_file "$counter"
  run cat "$counter"
  assert_output "x"

  pitchfork stop printer
}

# ---------------------------------------------------------------------------
# on_output – PITCHFORK_MATCHED_LINE env var
# ---------------------------------------------------------------------------

@test "on_output passes matched line via PITCHFORK_MATCHED_LINE" {
  local capture="$TEST_TEMP_DIR/matched_line"

  create_pitchfork_toml <<EOF
[daemons.printer]
run = "bash -c 'sleep 0.2; echo server started on port 8080; sleep 60'"

[daemons.printer.hooks]
on_output = { filter = "server started", run = "sh -c 'echo \$PITCHFORK_MATCHED_LINE > $capture'" }
EOF

  pitchfork supervisor start
  pitchfork start printer

  wait_for_file "$capture"
  run cat "$capture"
  assert_output --partial "server started on port 8080"

  pitchfork stop printer
}

# ---------------------------------------------------------------------------
# on_output – debounce prevents rapid re-firing
# ---------------------------------------------------------------------------

@test "on_output debounce limits firing rate" {
  local counter="$TEST_TEMP_DIR/debounce_count"

  # Emit 5 lines quickly then pause; debounce of 2s should collapse them into 1 firing.
  create_pitchfork_toml <<EOF
[daemons.spammer]
run = "bash -c 'for i in 1 2 3 4 5; do echo tick; done; sleep 60'"

[daemons.spammer.hooks]
on_output = { filter = "tick", run = "sh -c 'echo x >> $counter'", debounce = "2s" }
EOF

  pitchfork supervisor start
  pitchfork start spammer

  # Wait long enough for at least one firing but less than 3x the debounce window.
  sleep 2.5

  local count
  count=$(wc -l < "$counter" | tr -d ' ')
  # At least 1 firing must have occurred (lower bound).
  [[ "$count" -ge 1 ]]
  # With 5 rapid lines and a 2s debounce only 1–2 firings are expected (upper bound).
  [[ "$count" -le 2 ]]

  pitchfork stop spammer
}

# ---------------------------------------------------------------------------
# on_output – stderr is also monitored
# ---------------------------------------------------------------------------

@test "on_output fires on stderr output" {
  local marker="$TEST_TEMP_DIR/stderr_hook"

  create_pitchfork_toml <<EOF
[daemons.errorer]
run = "bash -c 'sleep 0.2; echo error: something went wrong >&2; sleep 60'"

[daemons.errorer.hooks]
on_output = { filter = "error:", run = "touch $marker" }
EOF

  pitchfork supervisor start
  pitchfork start errorer

  wait_for_file "$marker"
  assert_file_exists "$marker"

  pitchfork stop errorer
}
