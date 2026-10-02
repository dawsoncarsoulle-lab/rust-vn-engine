import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('notices', Path(__file__).with_name('sync_dependency_notices.py'))
notices = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notices)


class InventoryTests(unittest.TestCase):
    def package(self, root, name, source='registry+fixture', license='MIT'):
        folder = root / name
        folder.mkdir()
        (folder / 'LICENSE').write_text('Original upstream notice\n', encoding='utf-8')
        return dict(id=name, name=name, version='1.0.0', source=source,
                    license=license, license_file=None, manifest_path=str(folder / 'Cargo.toml'),
                    repository=None, authors=[])

    def metadata(self, root):
        packages = [self.package(root, 'app', source=None), self.package(root, 'used'), self.package(root, 'unrelated')]
        return dict(packages=packages, resolve=dict(nodes=[dict(id='app', dependencies=['used']), dict(id='used', dependencies=[])]))

    def test_dependency_closure_preserves_notices_and_is_idempotent(self):
        with tempfile.TemporaryDirectory() as directory:
            metadata = self.metadata(Path(directory))
            original = 'Previously distributed notices\n'
            added = notices.missing_notices(metadata, original, ['app'])
            self.assertEqual(len(added), 1)
            self.assertIn('used 1.0.0\nLicense: MIT', added[0])
            self.assertIn('Original upstream notice', added[0])
            self.assertNotIn('Authors: \n', added[0])
            self.assertEqual(notices.missing_notices(metadata, original + ''.join(added), ['app']), [])

    def test_unknown_licenses_require_manual_review(self):
        with tempfile.TemporaryDirectory() as directory:
            metadata = self.metadata(Path(directory))
            metadata['packages'][1]['license'] = None
            with self.assertRaisesRegex(ValueError, 'Manual license review'):
                notices.missing_notices(metadata, 'Keep me', ['app'])

    def test_notice_symlinks_cannot_escape_the_crate(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            metadata = self.metadata(root)
            (root / 'used' / 'NOTICE').symlink_to(root / 'unrelated' / 'LICENSE')
            with self.assertRaisesRegex(ValueError, 'Invalid dependency notice'):
                notices.missing_notices(metadata, '', ['app'])

    def test_missing_release_root_is_an_error(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, 'release packages'):
                notices.missing_notices(self.metadata(Path(directory)), '', ['missing'])


if __name__ == '__main__':
    unittest.main()
