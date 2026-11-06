# frozen_string_literal: true
require 'bundler/gem_tasks'
require 'rake/testtask'
require 'rb_sys/extensiontask'
GEMSPEC = Gem::Specification.load('window_machines.gemspec')
RbSys::ExtensionTask.new('window_machines_native', GEMSPEC) do |ext|
  ext.lib_dir = 'lib/window_machines_native'
  ext.tmp_dir = 'tmp/rb_sys'
end
Rake::TestTask.new(:test => :compile) do |test|
  test.libs << 'lib'
  test.pattern = 'test/**/*_test.rb'
end
task :rbs do
  sh 'rbs', '-I', 'sig', 'validate'
end
task default: %i[test rbs]

Dir[File.join(__dir__, 'tasks/*.rake')].sort.each { |f| load f }
