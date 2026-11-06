# window-machines

> **AMS Akidau**, NCC-4219, *Akidau*-class survey vessel.

Engine-room flight manual: divide observation time into bounded survey windows.

Allocation-free `no_std` window boundaries and one-key bounded session tracking.
All timestamps and widths use one caller-selected unsigned unit and origin.
Tumbling/sliding windows are aligned to zero and half-open `[start, end)`.
Sliding results are newest-start-first; too-small output capacity rejects rather
than silently truncating. Overflowing boundaries reject.

Watermarks never regress. A window closes when watermark reaches its end plus
allowed lateness; arithmetic overflow means it cannot yet close.

Sessions merge events whose inactivity gaps are **at most** the configured gap,
including out-of-order bridging events. Events at/below the current watermark
are late and rejected; session allowed lateness is zero in this version. Advancing
a watermark does not discard sessions: explicitly drain `take_closed()`.
Full tables reject events that cannot merge. Callers retain aggregation payloads
and apply the same merges; the core stores boundaries only. Multi-key indexing,
retractions, and distributed watermark coordination are not implemented.
