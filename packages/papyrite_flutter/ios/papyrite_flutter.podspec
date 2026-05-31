Pod::Spec.new do |s|
  s.name             = 'papyrite_flutter'
  s.version          = '0.1.0'
  s.summary          = 'Flutter FFI bindings for Papyrite.'
  s.description      = 'Loads the Papyrite Rust library through Dart FFI.'
  s.homepage         = 'https://example.invalid/papyrite'
  s.license          = { :type => 'MIT' }
  s.author           = { 'Papyrite' => 'papyrite@example.invalid' }
  s.source           = { :path => '.' }
  s.platform         = :ios, '12.0'
  s.dependency       'Flutter'
  s.vendored_frameworks = 'Frameworks/Papyrite.xcframework'
  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'EXCLUDED_ARCHS[sdk=iphonesimulator*]' => 'i386'
  }
end
