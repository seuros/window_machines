# frozen_string_literal: true

require_relative 'window_machines/version'

module WindowMachines
  # Half-open event-time interval: `start` is inside, `end` is not. Like the
  # core's `Window::new` it holds only Integers in 0..2**64-1 with
  # `start < end`: TypeError for another type, RangeError for another value.
  Window = Data.define(:start, :end) do
    def initialize(**)
      super
      WindowMachines.__send__(:__window, start, self.end)
    end
  end

  # Boundaries of a session; both endpoints are timestamps of real events.
  # Like the core's `Session::new` it holds only Integers in 0..2**64-1 with
  # `first <= last`: TypeError for another type, RangeError for another value.
  # `covers?(timestamp)` is native (the core's `covers`).
  Session = Data.define(:first, :last) do
    def initialize(**)
      super
      WindowMachines.__send__(:__session, first, last)
    end
  end

  # What Opened, Extended and Covered share: they hold a Session and nothing
  # else (TypeError).
  module Inserting
    def initialize(**)
      super
      WindowMachines.__send__(:__inserting, session)
    end
  end

  # The core's `Inserting`, what an honoured `Sessions#insert` did. Each
  # carries the session the event is in.
  #
  # Opened: the event joined no held session, a new one took a slot.
  Opened = Data.define(:session) { include Inserting }
  # Extended: the event grew one held session or bridged several into this one.
  Extended = Data.define(:session) { include Inserting }
  # Covered: a held session already covered the timestamp, nothing changed.
  Covered = Data.define(:session) { include Inserting }
end

# Native is required: no algorithm fallback. It validates and builds the Data
# values above, resolves their classes once at load and adds
# `Session#covers?`, so they must exist first.
require 'window_machines_native/window_machines_native'

module WindowMachines
  private_class_method :__window, :__session, :__inserting
  private_constant :Inserting

  # Sealed: no subclass can skip the check by overriding `initialize`, and no
  # bare `allocate` builds a value that never ran it. Sealing stops
  # accidents, not reflection, so the native side reads only instances of
  # exactly these classes and re-validates their members through the core.
  [Window, Session, Opened, Extended, Covered].each do |value|
    value.singleton_class.undef_method(:allocate)
    value.define_singleton_method(:inherited) { |_| raise TypeError, "#{value} cannot be subclassed" }
  end
end
