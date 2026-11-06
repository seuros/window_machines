# frozen_string_literal: true

require_relative 'lib/window_machines/version'
Gem::Specification.new do |spec|
  spec.name = 'window_machines'
  spec.version = WindowMachines::VERSION
  spec.authors = ['Abdelkader Boudih']
  spec.email = ['oss@seuros.com']
  spec.summary = 'Native event-time window boundaries for Ruby'
  spec.description = 'Initial allocation-free window policy core and thin native Ruby binding. ' \
                     'Caller owns payloads, clocks, and execution.'
  spec.homepage = 'https://github.com/seuros/window_machines'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 4.0.0'
  spec.metadata['source_code_uri'] = spec.homepage
  spec.metadata['rubygems_mfa_required'] = 'true'
  spec.metadata['cargo_crate_name'] = 'window_machines_native'
  spec.metadata['cargo_manifest_path'] = 'ext/window_machines_native/ffi/Cargo.toml'
  spec.files = Dir.glob(%w[
    lib/**/*.rb sig/**/*.rbs ext/**/*.{rs,rb,toml,md} Cargo.toml
    Cargo.lock README.md CHANGELOG.md LICENSE.txt
  ]).select { |p| File.file?(p) }
  spec.require_paths = ['lib']
  spec.extensions = ['ext/window_machines_native/extconf.rb']
  spec.add_dependency 'rb_sys', '~> 0.9', '>= 0.9.130'
end
