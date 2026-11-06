# frozen_string_literal: true
require 'minitest/autorun'
require 'window_machines'

class WindowMachinesTest < Minitest::Test
  def test_boundaries_and_sliding_limit
    assert_equal [10, 20], WindowMachines.tumbling(12, 10)
    assert_equal [[10, 20], [5, 15]], WindowMachines.sliding(12, 10, 5)
    assert_equal [], WindowMachines.sliding(12, 1, 5)
    assert_raises(RangeError) { WindowMachines.sliding(100, 100, 1) }
    assert_raises(RangeError) { WindowMachines.tumbling(2**64 - 1, 1) }
    assert_raises(ArgumentError) { WindowMachines.tumbling(0, 0) }
  end

  def test_strict_integer_validation
    [1.2, Float::INFINITY, Float::NAN, '2', Object.new].each do |bad|
      assert_raises(TypeError) { WindowMachines.tumbling(bad, 2) }
      assert_raises(TypeError) { WindowMachines.sliding(0, 2, bad) }
    end
    [-1, 2**64].each { |bad| assert_raises(RangeError) { WindowMachines.tumbling(bad, 1) } }
  end

  def test_watermark_and_freeze
    w = WindowMachines::Watermark.new
    assert_nil w.value
    refute w.closes?(0, 10, 0)
    assert w.advance(10)
    refute w.advance(9)
    assert_equal 10, w.value
    assert w.closes?(0, 10, 0)
    refute w.closes?(0, 10, 1)
    assert_raises(ArgumentError) { w.closes?(10, 10, 0) }
    assert_raises(ArgumentError) { w.closes?(0, 2**64 - 1, 1) }
    assert_raises(FrozenError) { w.freeze.advance(11) }
  end

  def test_session_bridges_closes_and_gc
    s = WindowMachines::Sessions.new(5)
    assert_equal [10, 10], s.insert(10)
    assert_equal [20, 20], s.insert(20)
    GC.start
    GC.compact
    assert_equal [10, 20], s.insert(15)
    assert s.advance_watermark(24)
    assert_nil s.take_closed
    assert s.advance_watermark(25)
    assert_equal [10, 20], s.take_closed
    assert_nil s.take_closed
    assert_raises(ArgumentError) { s.insert(25) }
    assert_raises(FrozenError) { s.freeze.take_closed }
    assert_raises(FrozenError) { s.advance_watermark(26) }
    assert_raises(FrozenError) { s.insert(30) }
  end

  def test_session_capacity_and_rejection
    s = WindowMachines::Sessions.new(1)
    32.times { |i| assert_equal [i * 3, i * 3], s.insert(i * 3) }
    assert_raises(RangeError) { s.insert(100) }
    assert_raises(RangeError) { s.insert(2**64 - 1) }
    assert_raises(TypeError) { s.advance_watermark(1.2) }
    assert s.advance_watermark(100)
    assert_equal 32, 32.times.count { s.take_closed }
    assert_nil s.take_closed
  end
end
