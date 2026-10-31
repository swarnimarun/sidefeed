import json, os, re
from playwright.sync_api import sync_playwright, expect
out=os.path.join(os.path.dirname(__file__),'screenshots');os.makedirs(out,exist_ok=True)
def capture(page, filename):
 # Never deliver a navigation/loading frame as a completed screen.
 expect(page.locator('#main')).to_be_visible()
 expect(page.locator('.loading')).to_have_count(0)
 expect(page.locator('#toast')).to_have_text('',timeout=7000)
 page.screenshot(path=os.path.join(out,filename),full_page=not filename.startswith('mobile-'))
def navigate(page, view):
 # Exercise real masthead, bottom-navigation or utility controls; never inject routes.
 anchors=page.locator('a[href="#'+view+'"]:visible')
 if anchors.count()==0:
  utilities=page.locator('details.utilities > summary')
  if not utilities.is_visible():
   page.get_by_role('button',name='Archive',exact=True).click()
  utilities.click()
  anchors=page.locator('a[href="#'+view+'"]:visible')
 expect(anchors.first).to_be_visible()
 anchors.first.click()
 expect(page).to_have_url(re.compile(r'#'+re.escape(view)+r'$'))

def check_protected_model_flow(browser):
 # Synthetic second node: no screenshots, real secrets, external requests or billing.
 token='qa-browser-owner-token-not-a-real-secret'
 headers={'Authorization':'Bearer '+token}
 tab=browser.new_page(viewport={'width':1440,'height':1000})
 tab_errors=[];tab.on('pageerror',lambda e:tab_errors.append(str(e)))
 tab.goto('http://127.0.0.1:8800')
 expect(tab.get_by_role('heading',name='Connect to your node',exact=True)).to_be_visible()
 def connect():
  tab.get_by_label('Owner token',exact=True).fill(token)
  tab.locator('#connection-form').get_by_role('button',name='Connect',exact=True).click()
  expect(tab.locator('#connection-form')).to_have_count(0)
  expect(tab.locator('.loading')).to_have_count(0)
  expect(tab.get_by_role('alert')).to_have_count(0)
 connect()
 assert tab.evaluate('(token) => !Object.values(localStorage).includes(token) && !Object.values(sessionStorage).includes(token)',token), 'Owner token persisted in browser storage'
 assert not tab.context.cookies(), 'Owner token must not create a cookie session'
 navigate(tab,'models')
 expect(tab.get_by_role('heading',name='Request a bounded model job',exact=True)).to_be_visible()
 tab.get_by_role('checkbox',name='Science and software fixture',exact=False).check()
 jobs_url='http://127.0.0.1:8800/api/v3/models/jobs'
 tab.once('dialog',lambda d:d.dismiss())
 tab.get_by_role('button',name='Review and approve one job…',exact=True).click()
 assert tab.request.get(jobs_url,headers=headers).json()['jobs']==[], 'Canceled model approval dispatched a job'
 def approve(dialog):
  assert 'Synthetic QA provider' in dialog.message
  assert '1 selected stories' in dialog.message
  assert 'private' in dialog.message
  dialog.accept()
 tab.once('dialog',approve)
 tab.get_by_role('button',name='Review and approve one job…',exact=True).click()
 expect(tab.get_by_text('succeeded',exact=True)).to_be_visible()
 jobs=tab.request.get(jobs_url,headers=headers).json()['jobs']
 assert len(jobs)==1 and jobs[0]['status']=='succeeded'
 assert jobs[0]['consent']['includePrivate'] is True and len(jobs[0]['itemIds'])==1
 assert tab.request.get('http://127.0.0.1:8800/api/v3/drafts',headers=headers).json()['drafts']==[], 'Model approval unexpectedly published or created drafts'
 tab.reload()
 expect(tab.get_by_role('heading',name='Connect to your node',exact=True)).to_be_visible()
 expect(tab.get_by_label('Owner token',exact=True)).to_have_value('')
 connect()
 tab.locator('details.utilities > summary').click()
 tab.get_by_role('button',name='Manage connection',exact=True).click()
 tab.get_by_role('button',name='Disconnect',exact=True).click()
 expect(tab.get_by_role('heading',name='Connect to your node',exact=True)).to_be_visible()
 expect(tab.get_by_label('Owner token',exact=True)).to_have_value('')
 assert not tab_errors,tab_errors
 tab.close()

with sync_playwright() as p:
 browser=p.chromium.launch(headless=True)
 page=browser.new_page(viewport={'width':1440,'height':1000}); errors=[]; requests=[];page.on('pageerror',lambda e:errors.append(str(e)));page.on('request',lambda r:requests.append((r.method,r.url)))
 page.goto('http://127.0.0.1:8799');expect(page.get_by_role('heading',name='Your reading starts here')).to_be_visible();capture(page,'desktop-empty.png')
 navigate(page,'sources');page.get_by_text('I already have the feed URL',exact=True).click();page.get_by_label('Feed URL',exact=True).fill('https://fixture.test/feed.json');page.get_by_label('Publication name',exact=True).fill('QA source');page.get_by_label('Feed format',exact=True).select_option('json');page.get_by_role('button',name='Subscribe',exact=True).click();expect(page.get_by_role('heading',name='Your publications 1',exact=True)).to_be_visible()
 page.get_by_text('Source controls & fetch history',exact=True).click();page.get_by_role('button',name='Fetch now',exact=True).click();expect(page.locator('#toast')).to_contain_text('Feed fetch completed')
 expect(page.get_by_role('heading',name='Sources',exact=True)).to_be_visible()
 expect(page.get_by_role('heading',name='Your publications 1',exact=True)).to_be_visible()
 expect(page.get_by_role('heading',name='QA source',exact=True)).to_be_visible()
 capture(page,'desktop-sources.png')
 navigate(page,'inbox');expect(page.locator('article.story')).to_have_count(3);expect(page.get_by_role('button',name='Science and software fixture',exact=True)).to_be_visible();page.get_by_role('article',name='Science and software fixture',exact=True).get_by_role('button',name='☆ Save',exact=True).click();expect(page.get_by_role('button',name='★ Saved',exact=True)).to_be_visible();page.get_by_role('article',name='Science and software fixture',exact=True).get_by_role('button',name='Mark read',exact=True).click();expect(page.get_by_role('button',name='Mark unread',exact=True)).to_be_visible();page.reload();expect(page.get_by_role('button',name='★ Saved',exact=True)).to_be_visible();page.get_by_role('button',name='Science and software fixture',exact=True).click();expect(page.get_by_text('A deterministic article for browser verification.',exact=False)).to_be_visible();page.get_by_role('button',name='← Back to stories',exact=True).click()
 context=page.get_by_role('complementary',name='Reading context',exact=True)
 expect(context.get_by_role('heading',name='Related reading',exact=True)).to_be_visible()
 expect(context.locator('.related-entry')).to_have_count(2)
 expect(context.get_by_text('Same source',exact=True)).to_be_visible()
 expect(context.get_by_role('link',name='Visit original feed ↗',exact=True)).to_have_attribute('href','https://fixture.test/feed.json')
 context.get_by_role('button',name='Designing a slower, more useful reading list',exact=True).click()
 expect(page.get_by_role('heading',name='Designing a slower, more useful reading list',exact=True)).to_be_visible()
 page.get_by_role('button',name='← Back to stories',exact=True).click()
 expect(page.locator('article.story')).to_have_count(3)
 capture(page,'desktop-reader.png')
 navigate(page,'search');page.get_by_label('Search titles, authors and story text',exact=True).fill('not-found-xyz');page.get_by_role('button',name='Search',exact=True).click();expect(page.get_by_role('heading',name='No matching stories')).to_be_visible();expect(page.get_by_label('Search titles, authors and story text',exact=True)).to_have_value('not-found-xyz');capture(page,'desktop-search-empty.png')
 navigate(page,'agents');page.get_by_label('Website URL',exact=True).fill('https://fixture.test/website');page.get_by_role('button',name='Discover feeds →').click();expect(page.get_by_text('awaiting_approval',exact=True)).to_be_visible()
 page.get_by_text('Inspect proposal & evidence',exact=True).click()
 expect(page.locator('details[open] > pre.code')).to_be_visible()
 expect(page.locator('details[open] > pre.code')).to_contain_text('fetch_public_url')
 expect(page.get_by_role('button',name='Approve subscription',exact=True)).to_be_visible()
 capture(page,'desktop-agent-approval.png')
 page.once('dialog',lambda d:d.dismiss());page.get_by_role('button',name='Approve subscription').click();expect(page.get_by_text('awaiting_approval',exact=True)).to_be_visible();page.once('dialog',lambda d:d.accept());page.get_by_role('button',name='Approve subscription').click();expect(page.get_by_text('approved',exact=True)).to_be_visible()
 page.get_by_label('Website URL',exact=True).fill('https://fixture.test/error');page.get_by_role('button',name='Discover feeds →').click();expect(page.get_by_text('failed',exact=True)).to_be_visible();expect(page.get_by_text('Deterministic fixture origin failure',exact=True)).to_be_visible()
 navigate(page,'compose');page.get_by_label('Title',exact=True).fill('QA private draft');page.get_by_label('Your words',exact=True).fill('Only publish after confirmation');page.get_by_role('button',name='Save private draft').click();expect(page.get_by_text('draft',exact=True)).to_be_visible()
 expect(page.get_by_text('QA private draft',exact=True)).to_be_visible()
 expect(page.get_by_role('button',name='Publish locally…',exact=True)).to_be_visible()
 assert page.request.get('http://127.0.0.1:8799/public/feed.json').json()['items']==[], 'Private draft appeared in public feed'
 capture(page,'desktop-publishing-private-draft.png')
 page.get_by_role('button',name='Edit',exact=True).click()
 page.get_by_label('Your words',exact=True).fill('Unsaved changes must not replace the private draft')
 page.once('dialog',lambda d:d.dismiss())
 page.get_by_role('button',name='Clear editor',exact=True).click()
 expect(page.get_by_label('Your words',exact=True)).to_have_value('Unsaved changes must not replace the private draft')
 page.once('dialog',lambda d:d.accept())
 page.get_by_role('button',name='Clear editor',exact=True).click()
 expect(page.get_by_label('Your words',exact=True)).to_have_value('')
 saved_drafts=page.request.get('http://127.0.0.1:8799/api/v3/drafts').json()['drafts']
 assert len(saved_drafts)==1 and saved_drafts[0]['content']=='Only publish after confirmation', 'Canceled editing changed persisted draft'
 page.once('dialog',lambda d:d.dismiss());page.get_by_role('button',name='Publish locally…').click();expect(page.get_by_text('draft',exact=True)).to_be_visible();page.once('dialog',lambda d:d.accept());page.get_by_role('button',name='Publish locally…').click();expect(page.get_by_text('published',exact=True)).to_be_visible()
 # Register an explicitly synthetic trusted peer using real fixture API state.
 # Its one-time bearer stays in memory and is never rendered or logged.
 fixture_source=page.request.get('http://127.0.0.1:8799/api/v3/sources').json()['sources'][0]
 sharing=page.request.patch('http://127.0.0.1:8799/api/v3/sources/'+fixture_source['id'],data={'share':True})
 assert sharing.ok, 'Fixture source sharing failed'
 enrolled=page.request.post('http://127.0.0.1:8799/api/v3/fleet/peers',data={'name':'Fixture peer — registration only','sourceIds':[fixture_source['id']],'confirm':True})
 assert enrolled.status==201, 'Fixture peer enrollment failed'
 fixture_peer=enrolled.json()
 leased=page.request.post('http://127.0.0.1:8799/api/v3/fleet/transport/lease',headers={'Authorization':'Bearer '+fixture_peer['token']},data={'sourceId':fixture_source['id']})
 assert leased.ok, 'Fixture peer lease failed'
 navigate(page,'fleet')
 expect(page.get_by_role('heading',name='Trusted fleet',exact=True)).to_be_visible()
 expect(page.get_by_text('Fixture peer — registration only',exact=True)).to_be_visible()
 expect(page.get_by_role('heading',name='Polling leases',exact=True)).to_be_visible()
 assert fixture_peer['token'] not in page.locator('body').inner_text(), 'Peer credential was rendered'
 capture(page,'desktop-fleet.png')
 navigate(page,'inbox');page.go_back();expect(page.get_by_role('heading',name='Trusted fleet',exact=True)).to_be_visible();page.go_forward();expect(page.get_by_role('heading',name='Today’s reading',exact=True)).to_be_visible()
 # Stage 2: disabled provider must remain inert; opening the screen is not consent.
 navigate(page,'models')
 expect(page.get_by_role('heading',name='AI & retrieval',exact=True)).to_be_visible()
 expect(page.get_by_text('Model processing is unavailable. Configure a provider on the server to enable it. No model calls will be made by this screen.',exact=True)).to_be_visible()
 expect(page.get_by_label('Operation',exact=True)).to_be_disabled()
 expect(page.get_by_role('button',name='Review and approve one job…',exact=True)).to_be_disabled()
 expect(page.get_by_role('checkbox',name='Science and software fixture',exact=False)).to_be_disabled()
 expect(page.get_by_text('No model jobs have been requested. Nothing is running automatically.',exact=True)).to_be_visible()
 assert not [r for r in requests if r[0]=='POST' and '/api/v3/models/jobs' in r[1]], 'Model dispatch occurred without consent'
 assert page.request.get('http://127.0.0.1:8799/api/v3/models/jobs').json()['jobs']==[]
 capture(page,'desktop-models-disabled.png')
 # Fetching a thread is an explicit action and the returned public graph is visibly partial.
 navigate(page,'threads')
 expect(page.get_by_role('heading',name='Follow the conversation',exact=True)).to_be_visible()
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
 capture(page,'desktop-thread-partial.png')
 navigate(page,'inbox')
 page.set_viewport_size({'width':390,'height':844})
 menu=page.get_by_role('button',name='Archive',exact=False)
 expect(menu).to_be_visible()
 expect(menu).to_have_attribute('aria-expanded','false')
 expect(page.get_by_role('navigation',name='Main navigation',exact=True)).to_be_hidden()
 menu.click();expect(menu).to_have_attribute('aria-expanded','true')
 page.keyboard.press('Escape');expect(menu).to_have_attribute('aria-expanded','false')
 expect(page.get_by_role('heading',name='Today’s reading',exact=True)).to_be_visible()
 lead=page.get_by_role('article',name='Science and software fixture',exact=True)
 lead.get_by_text('Related & source details',exact=False).click()
 expect(lead.get_by_role('heading',name='Related reading',exact=True)).to_be_visible()
 expect(lead.get_by_role('heading',name='Source provenance',exact=True)).to_be_visible()
 expect(lead.get_by_role('link',name='Visit original feed ↗',exact=True)).to_have_attribute('href','https://fixture.test/feed.json')
 expect(page.locator('article.story')).to_have_count(3)
 capture(page,'mobile-reader-context.png')
 lead.get_by_text('Related & source details',exact=False).click()
 expect(lead.get_by_role('heading',name='Source provenance',exact=True)).to_be_hidden()
 page.evaluate('window.scrollTo(0, 0)')
 expect(page.locator('article.story > h2')).to_have_count(3)
 capture(page,'mobile-reader.png')
 third_heading=page.locator('article.story > h2').nth(2).bounding_box()
 bottom_navigation=page.locator('.bottom-nav').bounding_box()
 print(json.dumps({'mobileThirdHeadline':third_heading,'mobileNavigation':bottom_navigation}))
 assert third_heading and bottom_navigation and third_heading['y']+third_heading['height'] <= bottom_navigation['y']-8, 'Third mobile headline hidden behind bottom navigation'
 capture(page,'mobile-reader.png')
 assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'Mobile horizontal overflow'
 navigate(page,'threads')
 expect(page.get_by_role('heading',name='Follow the conversation',exact=True)).to_be_visible()
 expect(page.get_by_label('Canonical DID-based AT URI',exact=True)).to_be_visible()
 assert len([r for r in requests if '/api/v3/threads?' in r[1]])==1, 'Mobile navigation triggered a thread fetch without consent'
 navigate(page,'inbox')
 expect(page.locator('article.story')).to_have_count(3)
 expect(page.get_by_role('navigation',name='Mobile navigation',exact=True)).to_be_visible()
 navigate(page,'sources');expect(menu).to_have_attribute('aria-expanded','false');expect(page.get_by_role('heading',name='Find a new publication',exact=True)).to_be_visible();expect(page.get_by_label('Website URL',exact=True)).to_be_visible();capture(page,'mobile-sources.png');assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'Mobile sources horizontal overflow'
 # Remaining implemented pages, captured after their actual data has rendered.
 page.set_viewport_size({'width':1440,'height':1000})
 navigate(page,'saved')
 expect(page.get_by_role('heading',name='Saved for later',exact=True)).to_be_visible()
 expect(page.get_by_role('button',name='Science and software fixture',exact=True)).to_be_visible()
 expect(page.get_by_role('button',name='★ Saved',exact=True)).to_be_visible()
 capture(page,'desktop-saved.png')
 navigate(page,'settings')
 expect(page.get_by_role('heading',name='Your node',exact=True)).to_be_visible()
 expect(page.get_by_role('heading',name='Publishing & deployment',exact=True)).to_be_visible()
 page.get_by_text('Live backend capabilities',exact=True).click()
 expect(page.locator('details[open] > pre.code')).to_contain_text('Sidefeed V3')
 capture(page,'desktop-settings-security.png')
 check_protected_model_flow(browser)
 assert not errors,errors
 print(json.dumps({'result':'PASS','pageerrors':errors,'screenshots':os.listdir(out)}));browser.close()
