import json, os
from playwright.sync_api import sync_playwright, expect
out=os.path.join(os.path.dirname(__file__),'screenshots');os.makedirs(out,exist_ok=True)
with sync_playwright() as p:
 browser=p.chromium.launch(headless=True)
 page=browser.new_page(viewport={'width':1440,'height':1000}); errors=[]; requests=[];page.on('pageerror',lambda e:errors.append(str(e)));page.on('request',lambda r:requests.append((r.method,r.url)))
 page.goto('http://127.0.0.1:8799');expect(page.get_by_role('heading',name='Make room for good reading')).to_be_visible();page.screenshot(path=out+'/desktop-empty.png',full_page=True)
 page.get_by_role('link',name='Feeds & discovery').click();page.get_by_label('Feed URL',exact=True).fill('https://fixture.test/feed.json');page.get_by_label('Name',exact=True).fill('QA source');page.get_by_label('Format',exact=True).select_option('json');page.get_by_role('button',name='+ Subscribe',exact=True).click();expect(page.get_by_text('Your sources (1)',exact=True)).to_be_visible()
 page.get_by_role('button',name='Fetch now',exact=True).click();expect(page.get_by_role('status')).to_contain_text('Feed fetch completed');page.get_by_role('link',name='Inbox',exact=True).click();expect(page.get_by_role('button',name='Science and software fixture',exact=True)).to_be_visible();page.get_by_role('button',name='☆ Save',exact=True).click();expect(page.get_by_role('button',name='★ Saved',exact=True)).to_be_visible();page.get_by_role('button',name='Mark read',exact=True).click();expect(page.get_by_role('button',name='Mark unread',exact=True)).to_be_visible();page.reload();expect(page.get_by_role('button',name='★ Saved',exact=True)).to_be_visible();page.get_by_role('button',name='Science and software fixture',exact=True).click();expect(page.get_by_text('A deterministic article for browser verification.',exact=False)).to_be_visible();page.get_by_role('button',name='← Back to stories',exact=True).click();page.screenshot(path=out+'/desktop-reader.png',full_page=True)
 page.get_by_role('link',name='Search',exact=True).click();page.get_by_label('Search stories').fill('not-found-xyz');page.get_by_role('button',name='Search',exact=True).click();expect(page.get_by_role('heading',name='No matching stories')).to_be_visible()
 page.get_by_role('link',name='Agent harness',exact=True).click();page.get_by_label('Public website URL').fill('https://fixture.test/website');page.get_by_role('button',name='Run discovery').click();expect(page.get_by_text('awaiting_approval',exact=True)).to_be_visible();page.once('dialog',lambda d:d.dismiss());page.get_by_role('button',name='Approve subscription').click();expect(page.get_by_text('awaiting_approval',exact=True)).to_be_visible();page.once('dialog',lambda d:d.accept());page.get_by_role('button',name='Approve subscription').click();expect(page.get_by_text('approved',exact=True)).to_be_visible()
 page.get_by_label('Public website URL').fill('https://fixture.test/error');page.get_by_role('button',name='Run discovery').click();expect(page.get_by_text('failed',exact=True)).to_be_visible();expect(page.get_by_text('Deterministic fixture origin failure',exact=True)).to_be_visible()
 page.get_by_role('link',name='Publishing',exact=True).click();page.get_by_label('Title',exact=True).fill('QA private draft');page.get_by_label('Your words',exact=True).fill('Only publish after confirmation');page.get_by_role('button',name='Save private draft').click();expect(page.get_by_text('draft',exact=True)).to_be_visible();page.once('dialog',lambda d:d.dismiss());page.get_by_role('button',name='Publish locally…').click();expect(page.get_by_text('draft',exact=True)).to_be_visible();page.once('dialog',lambda d:d.accept());page.get_by_role('button',name='Publish locally…').click();expect(page.get_by_text('published',exact=True)).to_be_visible()
 page.get_by_role('link',name='Trusted fleet',exact=True).click();expect(page.get_by_role('heading',name='Your node, for now')).to_be_visible();page.get_by_role('link',name='Inbox',exact=True).click();page.go_back();expect(page.get_by_role('heading',name='Trusted fleet',exact=True)).to_be_visible();page.go_forward();expect(page.get_by_role('heading',name='Your inbox',exact=True)).to_be_visible()
 # Stage 2: disabled provider must remain inert; opening the screen is not consent.
 page.get_by_role('link',name='AI & retrieval',exact=True).click()
 expect(page.get_by_role('heading',name='AI & retrieval',exact=True)).to_be_visible()
 expect(page.get_by_text('Model processing is unavailable. Configure a provider on the server to enable it. No model calls will be made by this screen.',exact=True)).to_be_visible()
 expect(page.get_by_label('Operation',exact=True)).to_be_disabled()
 expect(page.get_by_role('button',name='Review and approve one job…',exact=True)).to_be_disabled()
 expect(page.get_by_role('checkbox',name='Science and software fixture',exact=False)).to_be_disabled()
 expect(page.get_by_text('No model jobs have been requested. Nothing is running automatically.',exact=True)).to_be_visible()
 assert not [r for r in requests if r[0]=='POST' and '/api/v3/models/jobs' in r[1]], 'Model dispatch occurred without consent'
 assert page.request.get('http://127.0.0.1:8799/api/v3/models/jobs').json()['jobs']==[]
 page.screenshot(path=out+'/desktop-models-disabled.png',full_page=True)
 # Fetching a thread is an explicit action and the returned public graph is visibly partial.
 page.get_by_role('link',name='Inbox',exact=True).click()
 page.get_by_role('button',name='Science and software fixture',exact=True).click()
 expect(page.get_by_role('heading',name='Explore an AT Protocol thread',exact=True)).to_be_visible()
 assert not [r for r in requests if '/api/v3/threads?' in r[1]], 'Thread fetched before explicit request'
 page.get_by_label('Canonical DID-based AT URI',exact=True).fill('at://did:plc:qafixture/app.bsky.feed.post/root')
 page.get_by_role('button',name='Fetch thread',exact=True).click()
 expect(page.get_by_role('heading',name='AT Protocol thread snapshot',exact=True)).to_be_visible()
 expect(page.get_by_text('Partial snapshot',exact=True)).to_be_visible()
 expect(page.get_by_text('QA public thread root. Synthetic fixture, no external network.',exact=True)).to_be_visible()
 expect(page.get_by_text('Blocked post',exact=True)).to_be_visible()
 expect(page.get_by_text('Content was not supplied by this public snapshot. No missing text has been inferred.',exact=True)).to_be_visible()
 expect(page.get_by_role('link',name='Open original ↗',exact=True)).to_have_attribute('href','https://bsky.app/profile/did%3Aplc%3Aqafixture/post/root')
 assert len([r for r in requests if '/api/v3/threads?' in r[1]])==1, 'Thread request was repeated unexpectedly'
 page.screenshot(path=out+'/desktop-thread-partial.png',full_page=True)
 page.get_by_role('button',name='← Back to stories',exact=True).click()
 page.set_viewport_size({'width':390,'height':844});page.screenshot(path=out+'/mobile-reader.png',full_page=True);assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'Mobile horizontal overflow'
 page.get_by_role('link',name='Feeds & discovery').click();page.screenshot(path=out+'/mobile-sources.png',full_page=True);assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'Mobile sources horizontal overflow'
 assert not errors,errors
 print(json.dumps({'result':'PASS','pageerrors':errors,'screenshots':os.listdir(out)}));browser.close()
