# frozen_string_literal: true

require 'minitest/autorun'
require 'open3'
require 'rbconfig'

# The classes of the values the extension wraps (Watermark, Sessions) are
# fixed when it loads. Nothing a caller does later, redefining a constant or
# `Object.const_get`, reaches a value being built. Each case runs in a fresh
# process: the class is fixed once per process, at load.
class ClassLookupTest < Minitest::Test
  LIB = File.expand_path('../lib', __dir__)

  PRELUDE = <<~RUBY
    require 'window_machines'
    W = WindowMachines
  RUBY

  # Runs `body` after PRELUDE in a new Ruby; answers its last line of output.
  def isolated(body)
    out, err, status = Open3.capture3(RbConfig.ruby, '-I', LIB, '-e', PRELUDE + body)
    assert status.success?, "exited #{status.exitstatus}: #{err}"
    out.lines.last&.chomp
  end

  # Bug: a Watermark constant redefined before the first watermark made it an
  # instance of the new class, without any of the watermark's methods; the
  # copy Sessions#watermark answers too.
  def test_a_redefined_watermark_constant_does_not_change_the_watermark_class
    assert_equal 'true true advanced advanced', isolated(<<~RUBY)
      sessions = W::Sessions.new(5)
      original = W::Watermark
      W.send(:remove_const, :Watermark)
      W.const_set(:Watermark, Class.new)
      built = original.new
      copy = sessions.watermark
      puts [built.instance_of?(original), copy.instance_of?(original), built.advance(1), copy.advance(1)].join(' ')
    RUBY
  end

  # Bug: the same for Sessions.
  def test_a_redefined_sessions_constant_does_not_change_the_sessions_class
    assert_equal 'true 1', isolated(<<~RUBY)
      original = W::Sessions
      W.send(:remove_const, :Sessions)
      W.const_set(:Sessions, Class.new)
      sessions = original.new(5)
      sessions.insert(8)
      puts [sessions.instance_of?(original), sessions.size].join(' ')
    RUBY
  end

  # Bug: building the first value ran `Object.const_get`, a caller's Ruby code
  # in the middle of a native call.
  def test_building_a_value_runs_no_const_get
    assert_equal '0 Watermark Sessions Watermark', isolated(<<~RUBY)
      hits = 0
      Object.define_singleton_method(:const_get) do |*args|
        hits += 1
        super(*args)
      end
      sessions = W::Sessions.new(5)
      values = [W::Watermark.at(3), sessions, sessions.watermark]
      puts [hits, *values.map { |value| value.class.name.split('::').last }].join(' ')
    RUBY
  end

  # Bug: a redefined `Object.const_get` that raised hit an `unwrap` in the
  # extension: an uncatchable fatal, and the process exited.
  def test_a_raising_const_get_does_not_kill_the_process
    assert_equal 'advanced advanced', isolated(<<~RUBY)
      Object.define_singleton_method(:const_get) { |*| raise IOError, 'lookup ran mid-call' }
      sessions = W::Sessions.new(5)
      puts [sessions.advance_watermark(1), sessions.watermark.advance(2)].join(' ')
    RUBY
  end
end
