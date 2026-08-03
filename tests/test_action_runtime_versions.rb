#!/usr/bin/env ruby
# frozen_string_literal: true

require "psych"

root = File.expand_path("..", __dir__)
workflow_paths = Dir[File.join(root, ".github/workflows/*.{yml,yaml}")]

def walk(value, &block)
  case value
  when Hash
    yield value
    value.each_value { |child| walk(child, &block) }
  when Array
    value.each { |child| walk(child, &block) }
  end
end

violations = []
workflow_paths.each do |path|
  workflow = Psych.safe_load(File.read(path), aliases: true)
  walk(workflow) do |mapping|
    action = mapping["uses"]
    next unless action.is_a?(String) && action.start_with?("actions/checkout@")
    next if action.match?(/\Aactions\/checkout@v(?:[7-9]|[1-9][0-9]+)\z/)

    violations << "#{File.basename(path)} uses #{action}"
  end
end

abort "Action runtime contract failed:\n#{violations.join("\n")}" unless violations.empty?

puts "Action runtime version contract tests passed"
