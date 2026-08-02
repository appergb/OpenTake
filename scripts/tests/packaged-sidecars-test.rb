#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "json"
require "open3"
require "optparse"
require "pathname"
require "tmpdir"

ROOT = Pathname.new(__dir__).join("../..").expand_path
LOCK_PATH = ROOT.join("scripts/ffmpeg-sidecars.lock.json")
BIN_DIR = ROOT.join("src-tauri/binaries")

def assert(condition, message)
  raise message unless condition
end

def capture!(*command, env: {})
  stdout, stderr, status = Open3.capture3(env, *command)
  raise "command failed (#{command.join(' ')}): #{stderr}" unless status.success?

  stdout
end

def host_target
  capture!("rustc", "--print", "host-tuple").strip
end

def executable_name(tool, target)
  suffix = target.include?("windows") ? ".exe" : ""
  "#{tool}-#{target}#{suffix}"
end

def verify_reported_version(path, tool, record)
  output = capture!(path.to_s, "-version", env: { "PATH" => "" })
  reported = output.lines.first.to_s.split[2].to_s
  expected = record.fetch("version")
  assert(reported == expected || reported.start_with?("#{expected}-"),
         "unexpected #{tool} version: #{output.lines.first}")
  assert(!output.include?("--enable-nonfree"),
         "#{tool} enables nonfree components and cannot be redistributed")

  license = capture!(path.to_s, "-L", env: { "PATH" => "" })
  assert(!license.downcase.include?("not legally redistributable"),
         "#{tool} reports that it is not legally redistributable")
end

def verify_locked_binary(tool, target, lock)
  record = lock.fetch("targets").fetch(target).fetch(tool)
  path = BIN_DIR.join(executable_name(tool, target))
  assert(path.file? && !path.symlink?, "missing regular provisioned sidecar: #{path}")
  assert(Digest::SHA256.file(path).hexdigest == record.fetch("sha256"),
         "sidecar checksum mismatch: #{path}")
  verify_reported_version(path, tool, record)
  path
end

def smoke_media_pipeline(ffmpeg, ffprobe)
  Dir.mktmpdir("opentake-packaged-sidecars") do |dir|
    source = File.join(dir, "source.mp4")
    decoded = File.join(dir, "decoded.rgba")
    encoded = File.join(dir, "encoded.mp4")
    clean_env = { "PATH" => "" }

    capture!(ffmpeg.to_s, "-v", "error", "-f", "lavfi", "-i",
             "color=c=0x336699:s=64x36:r=6", "-t", "1", "-c:v", "mpeg4",
             "-y", source, env: clean_env)
    probe = JSON.parse(capture!(ffprobe.to_s, "-v", "error", "-of", "json",
                                "-show_streams", "-show_format", source,
                                env: clean_env))
    video = probe.fetch("streams").find { |stream| stream["codec_type"] == "video" }
    assert(video && video["width"] == 64 && video["height"] == 36,
           "ffprobe did not report the generated 64x36 video")

    capture!(ffmpeg.to_s, "-v", "error", "-i", source, "-frames:v", "1",
             "-f", "rawvideo", "-pix_fmt", "rgba", "-y", decoded,
             env: clean_env)
    assert(File.size(decoded) == 64 * 36 * 4, "decoded RGBA frame size is wrong")

    capture!(ffmpeg.to_s, "-v", "error", "-i", source, "-vf", "scale=32:18",
             "-c:v", "mpeg4", "-y", encoded, env: clean_env)
    encoded_probe = JSON.parse(capture!(ffprobe.to_s, "-v", "error", "-of", "json",
                                        "-show_streams", encoded, env: clean_env))
    encoded_video = encoded_probe.fetch("streams").find { |stream| stream["codec_type"] == "video" }
    assert(encoded_video && encoded_video["width"] == 32 && encoded_video["height"] == 18,
           "encoded smoke output is not 32x18")
  end
end

def packaged_paths(package, target)
  package = Pathname.new(package).expand_path
  if target.include?("apple-darwin")
    macos = package.join("Contents/MacOS")
    [macos.join("ffmpeg"), macos.join("ffprobe")]
  elsif target.include?("windows")
    [package.join("ffmpeg.exe"), package.join("ffprobe.exe")]
  else
    raise "packaged smoke is only defined for macOS and Windows"
  end
end

def packaged_macos_windows_sidecars_resolve_and_execute(package: nil)
  assert(LOCK_PATH.file?, "missing sidecar supply-chain lock: #{LOCK_PATH}")
  lock = JSON.parse(LOCK_PATH.read)
  target = host_target
  assert(lock.fetch("targets").key?(target), "unsupported packaged sidecar target: #{target}")

  %w[tauri.macos.conf.json tauri.windows.conf.json].each do |name|
    config = JSON.parse(ROOT.join("src-tauri", name).read)
    assert(config.dig("bundle", "externalBin") == %w[binaries/ffmpeg binaries/ffprobe],
           "#{name} must package the locked ffmpeg and ffprobe sidecars")
  end

  source_ffmpeg = verify_locked_binary("ffmpeg", target, lock)
  source_ffprobe = verify_locked_binary("ffprobe", target, lock)
  smoke_media_pipeline(source_ffmpeg, source_ffprobe)

  return unless package

  packaged_ffmpeg, packaged_ffprobe = packaged_paths(package, target)
  target_lock = lock.fetch("targets").fetch(target)
  assert(packaged_ffmpeg.file? && !packaged_ffmpeg.symlink? &&
         packaged_ffprobe.file? && !packaged_ffprobe.symlink?,
         "package does not contain ffmpeg and ffprobe beside the application executable")
  if target.include?("apple-darwin")
    # macOS code signing writes a Mach-O signature into each nested executable,
    # so a correctly signed final package will not retain the source SHA-256.
    # The source files were verified above; the final files must instead retain
    # their locked version and license metadata, execute the smoke, and satisfy
    # code-sign validation.
    capture!("codesign", "--verify", "--strict", packaged_ffmpeg.to_s)
    capture!("codesign", "--verify", "--strict", packaged_ffprobe.to_s)
  else
    assert(Digest::SHA256.file(packaged_ffmpeg).hexdigest == Digest::SHA256.file(source_ffmpeg).hexdigest,
           "packaged ffmpeg differs from the verified source sidecar")
    assert(Digest::SHA256.file(packaged_ffprobe).hexdigest == Digest::SHA256.file(source_ffprobe).hexdigest,
           "packaged ffprobe differs from the verified source sidecar")
  end
  verify_reported_version(packaged_ffmpeg, "ffmpeg", target_lock.fetch("ffmpeg"))
  verify_reported_version(packaged_ffprobe, "ffprobe", target_lock.fetch("ffprobe"))
  smoke_media_pipeline(packaged_ffmpeg, packaged_ffprobe)
end

options = { name: nil, package: nil }
OptionParser.new do |parser|
  parser.on("--name NAME") { |name| options[:name] = name }
  parser.on("--package PATH") { |path| options[:package] = path }
end.parse!

expected_name = "packaged_macos_windows_sidecars_resolve_and_execute"
assert(options[:name].nil? || options[:name] == expected_name,
       "unknown test name: #{options[:name]}")
packaged_macos_windows_sidecars_resolve_and_execute(package: options[:package])
puts "PASS: #{expected_name}"
