# frozen_string_literal: true

require_relative 'window_machines/version'

module WindowMachines
  # Half-open event-time interval: `start` is inside, `end` is not.
  Window = Data.define(:start, :end)

  # Boundaries of a session; both endpoints are timestamps of real events.
  Session = Data.define(:first, :last) do
    # Whether a timestamp lies between the endpoints, inclusive (the core's `covers`).
    def covers?(timestamp)
      raise TypeError, 'event time must be an Integer in 0..2**64' unless timestamp.is_a?(Integer)

      first <= timestamp && timestamp <= last
    end
  end

  # The core's `Inserting`, what an honoured `Sessions#insert` did. Each
  # carries the session the event is in.
  #
  # Opened: the event joined no held session, a new one took a slot.
  Opened = Data.define(:session)
  # Extended: the event grew one held session or bridged several into this one.
  Extended = Data.define(:session)
  # Covered: a held session already covered the timestamp, nothing changed.
  Covered = Data.define(:session)
end

# Native is required: no algorithm fallback. It builds the Data values above,
# so they must exist first.
require 'window_machines_native/window_machines_native'
