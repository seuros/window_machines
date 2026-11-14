# frozen_string_literal: true

require 'minitest/autorun'
require 'window_machines'

class WindowMachinesTest < Minitest::Test
  W = WindowMachines
  MAX = 2**64 - 1
  BAD_TYPES = [1.2, Float::INFINITY, Float::NAN, '2', nil, Object.new].freeze
  BAD_RANGES = [-1, 2**64].freeze

  def window(start, finish) = W::Window.new(start, finish)
  def session(first, last) = W::Session.new(first, last)

  def test_tumbling_answers_a_window_or_a_refusal
    assert_equal window(10, 20), W.tumbling(12, 10)
    assert_equal window(10, 20), W.tumbling(19, 10)
    assert_equal window(20, 30), W.tumbling(20, 10)
    assert_equal window(MAX - 1, MAX), W.tumbling(MAX - 1, 1)
    assert_equal :overflow, W.tumbling(MAX, 1)
    assert_equal :overflow, W.tumbling(MAX, 2)
  end

  def test_sliding_answers_windows_newest_first_or_a_refusal
    assert_equal [window(10, 20), window(5, 15)], W.sliding(12, 10, 5)
    assert_equal [window(5, 15), window(0, 10)], W.sliding(7, 10, 5)
    assert_equal [], W.sliding(12, 1, 5)
    assert_equal 32, W::CAPACITY
    assert_equal W::CAPACITY, W.sliding(100, 32, 1).size
    assert_equal :undersized, W.sliding(100, 33, 1)
    assert_equal :undersized, W.sliding(100, 100, 1)
    assert_equal :overflow, W.sliding(MAX, 2, 1)
  end

  def test_strict_integer_validation
    BAD_TYPES.each do |bad|
      assert_raises(TypeError) { W.tumbling(bad, 2) }
      assert_raises(TypeError) { W.tumbling(2, bad) }
      assert_raises(TypeError) { W.sliding(bad, 2, 2) }
      assert_raises(TypeError) { W.sliding(0, bad, 2) }
      assert_raises(TypeError) { W.sliding(0, 2, bad) }
      assert_raises(TypeError) { W::Sessions.new(bad) }
      assert_raises(TypeError) { W::Watermark.at(bad) }
    end
    BAD_RANGES.each do |bad|
      assert_raises(RangeError) { W.tumbling(bad, 1) }
      assert_raises(RangeError) { W.tumbling(1, bad) }
      assert_raises(RangeError) { W::Watermark.at(bad) }
    end
    # A zero span is out of range, not a refusal: the core cannot be asked.
    assert_raises(RangeError) { W.tumbling(0, 0) }
    assert_raises(RangeError) { W.sliding(0, 0, 1) }
    assert_raises(RangeError) { W.sliding(0, 1, 0) }
    assert_raises(RangeError) { W::Sessions.new(0) }
  end

  def test_data_values_mirror_the_core
    assert_equal %i[start end], W::Window.members
    assert_equal %i[first last], W::Session.members
    [W::Opened, W::Extended, W::Covered].each { |kind| assert_equal %i[session], kind.members }
    assert_equal 20, window(10, 20).end
    assert session(3, 5).covers?(3)
    assert session(3, 5).covers?(5)
    refute session(3, 5).covers?(2)
    refute session(3, 5).covers?(6)
  end

  def test_watermark_advances_repeats_and_refuses
    w = W::Watermark.new
    assert_nil w.value
    refute w.closes?(window(0, 10), 0)
    assert_equal :advanced, w.advance(10)
    assert_equal :repeated, w.advance(10)
    assert_equal :regressed, w.advance(9)
    assert_equal 10, w.value
    assert w.closes?(window(0, 10), 0)
    refute w.closes?(window(0, 10), 1)
    # Pure query, as the core answers it: only the end is read, and a closing
    # point past the end of event time is never reached.
    assert w.closes?(window(9, 10), 0)
    refute w.closes?(window(0, MAX), 1)
    assert_equal :advanced, w.advance(MAX)
    assert w.closes?(window(0, MAX), 0)
  end

  def test_watermark_at_and_strictness
    w = W::Watermark.at(8)
    assert_equal 8, w.value
    assert_equal :repeated, w.advance(8)
    assert_equal :regressed, w.advance(7)
    assert_equal 0, W::Watermark.at(0).value
    assert_raises(ArgumentError) { w.closes?(0, 10, 0) }
    assert_raises(TypeError) { w.closes?([0, 10], 0) }
    assert_raises(TypeError) { w.closes?(session(0, 10), 0) }
    assert_raises(TypeError) { w.closes?(window(0, 1.5), 0) }
    assert_raises(RangeError) { w.closes?(window(0, -1), 0) }
    BAD_TYPES.each do |bad|
      assert_raises(TypeError) { w.advance(bad) }
      assert_raises(TypeError) { w.closes?(window(0, 10), bad) }
    end
    BAD_RANGES.each { |bad| assert_raises(RangeError) { w.advance(bad) } }
    assert_equal 8, w.value
    w.freeze
    assert_raises(FrozenError) { w.advance(11) }
    assert_raises(FrozenError) { w.advance(8) }
    # Frozen is checked before the argument is read.
    (BAD_TYPES + BAD_RANGES).each { |bad| assert_raises(FrozenError) { w.advance(bad) } }
    assert_equal 8, w.value
    assert w.closes?(window(0, 8), 0)
  end

  def test_sessions_open_extend_bridge_and_close
    s = W::Sessions.new(5)
    assert_equal [5, 32, 0, true], [s.gap, s.capacity, s.size, s.empty?]
    assert_nil s.watermark.value
    assert_equal W::Opened.new(session(10, 10)), s.insert(10)
    assert_equal W::Opened.new(session(20, 20)), s.insert(20)
    GC.start
    GC.compact
    assert_equal 2, s.size
    assert_equal W::Extended.new(session(10, 20)), s.insert(15)
    assert_equal session(10, 20), s.insert(15).session
    assert_equal 1, s.size
    assert_equal :advanced, s.advance_watermark(24)
    assert_equal :repeated, s.advance_watermark(24)
    assert_equal :regressed, s.advance_watermark(23)
    assert_equal 24, s.watermark.value
    assert_nil s.take_closed
    assert_equal :advanced, s.advance_watermark(25)
    assert_equal session(10, 20), s.take_closed
    assert_nil s.take_closed
    assert_predicate s, :empty?
  end

  def test_covered_timestamps_are_honoured_while_the_session_is_held
    s = W::Sessions.new(5)
    [0, 5, 10].each { |at| s.insert(at) }
    covered = W::Covered.new(session(0, 10))
    [0, 5, 7, 10].each { |at| assert_equal covered, s.insert(at) }
    assert_equal :advanced, s.advance_watermark(15)
    assert_equal covered, s.insert(5)
    assert_equal :late, s.insert(11)
    assert_equal :late, s.insert(15)
    assert_equal 1, s.size
    assert_equal session(0, 10), s.take_closed
    # Drained: no history is kept, so the replay is late now.
    assert_equal :late, s.insert(5)
    assert_predicate s, :empty?
  end

  def test_session_refusals_change_nothing
    s = W::Sessions.new(1)
    W::CAPACITY.times { |i| assert_equal W::Opened.new(session(i * 3, i * 3)), s.insert(i * 3) }
    assert_equal :full, s.insert(100)
    assert_equal :overflow, s.insert(MAX)
    assert_equal W::CAPACITY, s.size
    assert_equal W::Extended.new(session(0, 1)), s.insert(1)
    assert_equal :advanced, s.advance_watermark(100)
    assert_equal :regressed, s.advance_watermark(99)
    assert_equal W::CAPACITY, s.size
    closed = Array.new(W::CAPACITY) { s.take_closed }
    assert_equal session(0, 1), closed.first
    assert_equal closed.sort_by(&:first), closed
    assert_nil s.take_closed
  end

  def test_sessions_watermark_is_a_copy
    s = W::Sessions.new(5)
    assert_equal :advanced, s.advance_watermark(7)
    copy = s.watermark
    assert_instance_of W::Watermark, copy
    assert_equal :advanced, copy.advance(50)
    assert_equal 7, s.watermark.value
    assert_equal W::Opened.new(session(8, 8)), s.insert(8)
  end

  def test_sessions_strictness_and_freeze
    s = W::Sessions.new(5)
    BAD_TYPES.each do |bad|
      assert_raises(TypeError) { s.insert(bad) }
      assert_raises(TypeError) { s.advance_watermark(bad) }
    end
    BAD_RANGES.each do |bad|
      assert_raises(RangeError) { s.insert(bad) }
      assert_raises(RangeError) { s.advance_watermark(bad) }
    end
    assert_predicate s, :empty?
    assert_raises(TypeError) { W::Sessions.allocate }
    assert_raises(TypeError) { W::Watermark.allocate }
    s.insert(1)
    s.freeze
    assert_raises(FrozenError) { s.take_closed }
    assert_raises(FrozenError) { s.advance_watermark(26) }
    assert_raises(FrozenError) { s.insert(30) }
    assert_raises(FrozenError) { s.insert(1) }
    (BAD_TYPES + BAD_RANGES).each do |bad|
      assert_raises(FrozenError) { s.insert(bad) }
      assert_raises(FrozenError) { s.advance_watermark(bad) }
    end
    assert_equal [1, nil], [s.size, s.watermark.value]
  end
end
