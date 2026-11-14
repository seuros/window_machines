# frozen_string_literal: true

require 'minitest/autorun'
require 'window_machines'

# Values are valid or do not exist, and the binding never loses work.
class ValuesTest < Minitest::Test
  W = WindowMachines
  MAX = 2**64 - 1
  BAD_TYPES = [1.2, Float::NAN, '2', nil, Object.new].freeze
  BAD_RANGES = [-1, 2**64].freeze

  # Replaces `klass.new` with `body`, handed the original `new`, for the length
  # of the block. (A Data class defines `new` on its own singleton class, so
  # the original is set aside and put back rather than removed.)
  def redefining(klass, body)
    singleton = klass.singleton_class
    singleton.alias_method(:__original_new, :new)
    singleton.remove_method(:new)
    original = klass.method(:__original_new)
    klass.define_singleton_method(:new) { |*args| body.call(original, *args) }
    yield
  ensure
    singleton.remove_method(:new)
    singleton.alias_method(:new, :__original_new)
    singleton.remove_method(:__original_new)
  end

  # The leaked constant of a named subclass, with `initialize` bypassed.
  def forged(name, parent)
    assert_raises(TypeError) { self.class.class_eval("class #{name} < #{parent}; end", __FILE__, __LINE__) }
    self.class.const_get(name).tap do |leaked|
      leaked.class_eval { def initialize(**kw) = Data.instance_method(:initialize).bind_call(self, **kw) }
    end
  end

  def failing(klass, &) = redefining(klass, ->(*) { raise IOError, 'boom' }, &)

  def test_a_window_is_held_to_the_core_constructor
    assert_equal [10, 20], W::Window.new(10, 20).deconstruct
    assert_equal W::Window.new(10, 20), W::Window.new(start: 10, end: 20)
    assert_equal MAX, W::Window.new(0, MAX).end
    assert_raises(RangeError) { W::Window.new(20, 10) }
    assert_raises(RangeError) { W::Window.new(10, 10) }
    assert_raises(RangeError) { W::Window.new(start: 20, end: 10) }
    assert_raises(RangeError) { W::Window.new(10, 20).with(end: 5) }
    BAD_TYPES.each do |bad|
      assert_raises(TypeError) { W::Window.new(bad, 10) }
      assert_raises(TypeError) { W::Window.new(0, bad) }
    end
    BAD_RANGES.each do |bad|
      assert_raises(RangeError) { W::Window.new(bad, MAX) }
      assert_raises(RangeError) { W::Window.new(0, bad) }
    end
    assert_raises(ArgumentError) { W::Window.new(1) }
    assert_raises(ArgumentError) { W::Window.new(1, 2, 3) }
    refute_respond_to W, :__window
  end

  def test_a_session_is_held_to_the_core_constructor
    assert_equal [3, 5], W::Session.new(3, 5).deconstruct
    assert_equal W::Session.new(7, 7), W::Session.new(first: 7, last: 7)
    assert_raises(RangeError) { W::Session.new(5, 3) }
    assert_raises(RangeError) { W::Session.new(3, 5).with(first: 6) }
    BAD_TYPES.each do |bad|
      assert_raises(TypeError) { W::Session.new(bad, 10) }
      assert_raises(TypeError) { W::Session.new(0, bad) }
      assert_raises(TypeError) { W::Session.new(3, 5).covers?(bad) }
    end
    BAD_RANGES.each do |bad|
      assert_raises(RangeError) { W::Session.new(bad, MAX) }
      assert_raises(RangeError) { W::Session.new(0, bad) }
      assert_raises(RangeError) { W::Session.new(3, 5).covers?(bad) }
    end
    assert_raises(ArgumentError) { W::Session.new(1) }
    refute_respond_to W, :__session
  end

  def test_an_inserting_value_holds_a_session_and_nothing_else
    held = W::Session.new(3, 5)
    [W::Opened, W::Extended, W::Covered].each do |kind|
      assert_equal held, kind.new(held).session
      assert_equal kind.new(held), kind.new(session: held)
      [nil, 3, [3, 5], W::Window.new(3, 5), Data.define(:first, :last).new(3, 5)].each do |bad|
        assert_raises(TypeError) { kind.new(bad) }
      end
      assert_raises(ArgumentError) { kind.new }
    end
    refute_respond_to W, :__inserting
    assert_raises(NameError) { W::Inserting }
  end

  def test_take_closed_keeps_the_session_when_building_it_raises
    s = W::Sessions.new(5)
    s.insert(10)
    s.advance_watermark(100)
    failing(W::Session) { assert_raises(IOError) { s.take_closed } }
    assert_equal 1, s.size
    assert_equal W::Session.new(10, 10), s.take_closed
    assert_nil s.take_closed
  end

  def test_insert_changes_nothing_when_building_the_answer_raises
    s = W::Sessions.new(5)
    assert_equal W::Opened.new(W::Session.new(10, 10)), s.insert(10)
    failing(W::Session) { assert_raises(IOError) { s.insert(30) } }
    failing(W::Opened) { assert_raises(IOError) { s.insert(30) } }
    failing(W::Extended) { assert_raises(IOError) { s.insert(12) } }
    assert_equal 1, s.size
    assert_equal W::Covered.new(W::Session.new(10, 10)), s.insert(10)
    assert_equal W::Extended.new(W::Session.new(10, 12)), s.insert(12)
    assert_equal W::Opened.new(W::Session.new(30, 30)), s.insert(30)
  end

  def test_a_reentrant_change_while_building_raises_and_the_outer_call_is_not_applied
    s = W::Sessions.new(5)
    advance = ->(to) { ->(original, *args) { s.advance_watermark(to).then { original.call(*args) } } }
    redefining(W::Opened, advance.(5)) { assert_raises(RuntimeError) { s.insert(10) } }
    assert_equal [0, 5], [s.size, s.watermark.value]
    assert_equal W::Opened.new(W::Session.new(10, 10)), s.insert(10)
    s.advance_watermark(100)
    redefining(W::Session, advance.(200)) { assert_raises(RuntimeError) { s.take_closed } }
    assert_equal [1, 200], [s.size, s.watermark.value]
    assert_equal W::Session.new(10, 10), s.take_closed
  end

  def test_a_freeze_while_building_raises_and_the_call_is_not_applied
    s = W::Sessions.new(5)
    freeze = ->(original, *args) { s.freeze && original.call(*args) }
    redefining(W::Opened, freeze) { assert_raises(FrozenError) { s.insert(10) } }
    assert_equal [0, true], [s.size, s.frozen?]
    t = W::Sessions.new(5)
    t.insert(10)
    t.advance_watermark(100)
    freeze = ->(original, *args) { t.freeze && original.call(*args) }
    redefining(W::Session, freeze) { assert_raises(FrozenError) { t.take_closed } }
    assert_equal 1, t.size
  end

  # A singleton class opened on a genuine value does not change what it is:
  # every native read accepts it, a clone of one included.
  def test_an_opened_singleton_class_is_still_the_gems_value
    window = W::Window.new(0, 10).tap(&:singleton_class)
    assert W::Watermark.at(10).closes?(window, 0)
    assert W::Watermark.at(10).closes?(window.clone, 0)
    held = W::Session.new(3, 5).tap(&:singleton_class)
    assert held.covers?(4)
    assert_equal held, W::Opened.new(held).session
  end

  # Members are read from the struct itself: a redefined reader never runs.
  def test_a_redefined_reader_is_ignored
    window = W::Window.new(0, 10)
    session = W::Session.new(3, 5)
    raising_readers(W::Window => %i[start end], W::Session => %i[first last]) do
      assert W::Watermark.at(10).closes?(window, 0)
      refute W::Watermark.at(9).closes?(window, 0)
      assert session.covers?(4)
      refute session.covers?(6)
      assert_raises(IOError) { session.first }
      assert_equal [3, 5], W::Opened.new(session).session.deconstruct
    end
  end

  # Replaces each listed reader with one that raises, for the block.
  def raising_readers(readers)
    readers.each do |klass, names|
      names.each do |name|
        klass.alias_method(:"__original_#{name}", name)
        klass.define_method(name) { raise IOError, "#{name} ran" }
      end
    end
    yield
  ensure
    readers.each do |klass, names|
      names.each do |name|
        klass.alias_method(name, :"__original_#{name}")
        klass.remove_method(:"__original_#{name}")
      end
    end
  end

  def test_the_extension_alone_is_a_name_error
    lib = File.expand_path('../lib', __dir__)
    script = 'begin; require "window_machines_native/window_machines_native"; ' \
             'rescue NameError => e; print e.class; end'
    output = IO.popen([RbConfig.ruby, '-I', lib, '-e', script], err: %i[child out], &:read)
    assert_equal 'NameError', output
  end

  def test_value_classes_are_resolved_at_load
    original = W.send(:remove_const, :Session)
    W.const_set(:Session, Data.define(:first, :last))
    GC.compact # the resolved classes are pinned
    assert_instance_of original, W::Sessions.new(5).insert(10).session
  ensure
    W.send(:remove_const, :Session)
    W.const_set(:Session, original)
  end

  VALUES = [W::Window, W::Session, W::Opened, W::Extended, W::Covered].freeze

  def test_values_are_sealed
    VALUES.each do |value|
      assert_raises(NoMethodError) { value.allocate }
      assert_raises(TypeError) { Class.new(value) { def initialize(**) = nil } }
    end
    # A named subclass raises too, but Ruby binds the constant before
    # `inherited` runs, so the constant leaks. The native side refuses it.
    assert_raises(TypeError) { self.class.class_eval('class LeakedSession < WindowMachines::Session; end') }
    assert self.class.const_defined?(:LeakedSession, false)
  end

  # Sealing stops accidents, not reflection: a leaked subclass binding
  # `Data#initialize` directly, or a Marshal stream with edited members,
  # builds an invalid value. The native boundary refuses both.
  def test_forged_values_never_reach_the_core
    session = forged(:ForgedSession, 'WindowMachines::Session').new(first: 9, last: 3)
    window = forged(:ForgedWindow, 'WindowMachines::Window').new(start: 9, end: 3)
    [W::Opened, W::Extended, W::Covered].each { |kind| assert_raises(TypeError) { kind.new(session) } }
    assert_raises(TypeError) { session.covers?(4) }
    assert_raises(TypeError) { W::Watermark.at(100).closes?(window, 0) }
    marshalled = Marshal.dump(W::Session.new(3, 9)).sub("firsti\b", "firsti\x0E".b).sub("lasti\x0E".b, "lasti\b")
    edited = Marshal.load(marshalled) # rubocop:disable Security/MarshalLoad
    assert_equal [9, 3], edited.deconstruct
    assert_raises(RangeError) { edited.covers?(4) }
    assert_raises(RangeError) { W::Opened.new(edited) }
    marshalled = Marshal.dump(W::Window.new(3, 9)).sub("starti\b", "starti\x0E".b).sub("endi\x0E".b, "endi\b")
    edited = Marshal.load(marshalled) # rubocop:disable Security/MarshalLoad
    assert_raises(RangeError) { W::Watermark.at(100).closes?(edited, 0) }
  end
end
