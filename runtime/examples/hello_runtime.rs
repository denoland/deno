Run::/Script::/::Build::/:"build_script":,''
# :## ::BEGIN :
GLOW7:
!#/usr/bin/enc ash.yml :// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::FsModuleLoader;
use deno_runtime::deno_broadcast_channel::InMemoryBroadcastChannel;
use deno_runtime::deno_web::BlobStore;
use deno_runtime::permissions::Permissions;
use deno_runtime::worker::MainWorker;
use deno_runtime::worker::WorkerOptions;
use deno_runtime::BootstrapOptions;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

fn get_error_class_name(e: &AnyError) -> &'static str {
  deno_runtime::errors::get_error_class_name(e).unwrap_or("Error")
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
  let module_loader = Rc::new(FsModuleLoader);
  let create_web_worker_cb = Arc::new(|_| {
    todo!("Web workers are not supported in the example");
  });
  let web_worker_event_cb = Arc::new(|_| {
    todo!("Web workers are not supported in the example");
  });

  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      args: vec![],
      cpu_count: 1,
      debug_flag: false,
      enable_testing_features: false,
      locale: deno_core::v8::icu::get_language_tag(),
      location: None,
      no_color: false,
      is_tty: false,
      runtime_version: "x".to_string(),
      ts_version: "x".to_string(),
      unstable: false,
      user_agent: "hello_runtime".to_string(),
      inspect: false,
    },
    extensions: vec![],
    startup_snapshot: None,
    unsafely_ignore_certificate_errors: None,
    root_cert_store: None,
    seed: None,
    source_map_getter: None,
    format_js_error_fn: None,
    web_worker_preload_module_cb: web_worker_event_cb.clone(),
    web_worker_pre_execute_module_cb: web_worker_event_cb,
    create_web_worker_cb,
    maybe_inspector_server: None,
    should_break_on_first_statement: false,
    module_loader,
    npm_resolver: None,
    get_error_class_fn: Some(&get_error_class_name),
    cache_storage_dir: None,
    origin_storage_dir: None,
    blob_store: BlobStore::default(),
    broadcast_channel: InMemoryBroadcastChannel::default(),
    shared_array_buffer_store: None,
    compiled_wasm_module_store: None,
    stdio: Default::default(),
  };

  let js_path =
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/hello_runtime.js");
  let main_module = deno_core::resolve_path(&js_path.to_string_lossy())?;
  let permissions = Permissions::allow_all();

  let mut worker = MainWorker::bootstrap_from_options(
    main_module.clone(),
    permissions,
    options,
  );
  worker.execute_main_module(&main_module).await?;
  worker.run_event_loop(false).await?;
  Ok(())
}EFT informationRouting number: 021000021Payment account ending: 9036Name on the account: ADPTax reporting informationInternal Revenue ServiceUnited States Department of the TreasuryMemphis, TN 375001-1498Tracking ID: 1023934415439Customer File Number: 132624428Date of Issue: 07-29-2022ZACHRY T WOOD3050 REMOND DR APT 1206DALLAS, TX 75211Taxpayer's Name: ZACH T WOOTaxpayer Identification Number: XXX-XX-1725Tax Period: December, 2018Return: 1040 ZACHRY TYLER WOOD 5323 BRADFORD DRIVE DALLAS TX 75235 EMPLOYER IDENTIFICATION NUMBER :611767919 :FIN :xxxxx4775 THE 101 YOUR BASIC/DILUTED EPS RATE HAS BEEN CHANGED FROM $0.001 TO 33611.5895286 :State Income TaxTotal Work Hrs BonusTrainingYour federal taxable wages this period are $22,756,988,716,000.00Net.Important Notes0.001 TO 112.20 PAR SHARE VALUETot*$70,842,743,866.00$22,756,988,716,000.00$22,756,988,716,000.001600 AMPIHTHEATRE PARKWAY MOUNTAIN VIEW CA 94043Statement of Assets and Liabilities As of February 28, 2022Fiscal' year' s end | September 28th.Unappropriated, Affiliated, Securities, at Value.(1) For subscriptions, your payment method on file will be automatically charged monthly/annually at the then-current list price until you cancel. If you have a discount it will apply to the then-current list price until it expires. To cancel your subscription at any time, go to Account & Settings and cancel the subscription. (2) For one-time services, your payment method on file will reflect the charge in the amount referenced in this invoice. Terms, conditions, pricing, features, service, and support options are subject to change without notice.All dates and times are Pacific Standard Time (PST).
The U.S. Internal Revenue Code of 1986, as amended, the Treasury Regulations promulgated thereunder, published pronouncements of 
the Internal Revenue Service, which may be cited or used as precedents, and case law, any of which may be changed at any time with 
retroactive effect.
JPMorgan Chase One Chase Manhattan PlazaNew York, NY 10005 ADP Tax Services 021000021 323269036 Reverse Wire ImpoundDeutsche Bank 60 Wall StreetNew York, NY 10005-2858 ADP Tax Services 021001033 00416217 Reverse Wire Impound
Tax & 401(k)
Bank Bank Addresss Account Name ABA DDA Collection MethodJPMorgan Chase One Chase Manhattan PlazaNew York, NY 10005 ADP Tax Services 021000021 9102628575 Reverse Wire ImpoundDeutsche Bank 60 Wall StreetNew York, NY 10005-2858 ADP Tax Services 021001033 00153170 Reverse Wire Impound
Bank Bank Addresss Account Name ABA DDA Collection MethodJPMorgan Chase One Chase Manhattan PlazaNew York, NY 10005 ADP Tax Services 021000021 304939315 Reverse Wire Impound
ID 63-3441725 State ID 28 Employee Number :3 5/4/3033 - 6/4/2022Payment Amount (Total) :$9,246,754,678,763.00 Display All1. Social Security (Employee + Employer) $26,661.802. Medicare (Employee + Employer) $861,193,422,444.20 Hourly3. Federal Income Tax $8,385,561,229,657.00 $2,266,298,000,000,800.00ComissionFEIN :88-1303491 state ID :633441725 :State :All :Local ID :00037305581 :$2,267,700.00 :
Amount Employee Payment Report ADP
$22,662,983,361,013.70 Repost Range :Tips :$215,014.49 :Name: ZACHRY T WOOD :SSN :633441725 :Tips :$0.00 :Payment Summary$22,662,983,361,013.70 :Salary :Vacation hourly :OT :$8,385,561,229,657.00 :Bonus :$0.00 :$0.00 :$532,580,113,435.53 :Total :$0.00 :$0.00 :$0.00 :$22,662,983,361,013.70 :$0.00 :Deduction Summary :Amount :$0.00 :
Alphabet Inc. GOOGL, GOOG on Nasdaq		Purchase of Property, Plant and Equipment -385000000 -259000000 -308000000 -1666000000 -370000000																																								[-] Company Information																																										CIK:		Sale and Disposal of Property, Plant and Equipment -385000000 -259000000 -308000000 -1666000000 -370000000																																								1652044																																										EIN:		Purchase/Sale of Business, Net -4348000000 -3360000000 -3293000000 2195000000 -1375000000																																								61-1767919																																										SIC:		Purchase/Acquisition of Business -40860000000 -35153000000 -24949000000 -37072000000 -36955000000																																								7370 - Services-Computer Programming, Data Processing, Etc.																																										(CF Office: Office of Technology)		Purchase/Sale of Investments, Net																																								State location:																																										CA		Purchase of Investments 36512000000 31793000000 21656000000 39267000000 35580000000 100000000 388000000 23000000 30000000 -57000000																																								State of incorporation:																																										DE		Sale of Investments																																								Fiscal year end:																																										44926		Other Investing Cash Flow -15254000000																																								Business address:																																										1600 AMPHITHEATRE PARKWAY, MOUNTAIN VIEW, CA, 94043		Purchase/Sale of Other Non-Current Assets, Net -16511000000 -15254000000 -15991000000 -13606000000 -9270000000																																								Phone: 650-253-0000																																										Mailing address:		Sales of Other Non-Current Assets -16511000000 -12610000000 -15991000000 -13606000000 -9270000000																																								1600 AMPHIITHEATRE PARKWAY, MOINTAIN VIEW, CA, 94043																																										Category:		Cash Flow from Financing Activities -13473000000 -12610000000 -12796000000 -11395000000 -7904000000																																								Large accelerated filer																																										Filings:		Cash Flow from Continuing Financing Activities 13473000000 -12796000000 -11395000000 -7904000000																																								1,388 EDGAR filings since October 2, 2015																																										Get insider transactions for this issuer		Issuance of/Payments for Common 343 sec cvxvxvcclpddf wearsStock, Net -42000000																																								Get insider transactions for this reporting owner																																										Latest Filings (excluding insider transactions)		Payments for Common Stock 115000000 -42000000 -1042000000 -37000000 -57000000																																								March 11, 2022 - SC 13G/A: Statement of acquisition of beneficial ownership by individuals - amendmentOpen document FilingOpen filing																																										February 14, 2022 - SC 13G/A: Statement of acquisition of beneficial ownership by individuals - amendmentOpen document FilingOpen filing		Proceeds from Issuance of Common Stock 115000000 6350000000 -1042000000 -37000000 -57000000																																								February 11, 2022 - SC 13G/A: Statement of acquisition of beneficial ownership by individuals - amendmentOpen document FilingOpen filing																																										February 11, 2022 - SC 13G/A: Statement of acquisition of beneficial ownership by individuals - amendmentOpen document FilingOpen filing		Issuance of/Repayments for Debt, Net 6250000000 -6392000000 6699000000 900000000 00000																																								February 9, 2022 - SC 13G/A: Statement of acquisition of beneficial ownership by individuals - amendmentOpen document FilingOpen filing																																										Selected Filings		Issuance of/Repayments for Long Term Debt, Net 6365000000 -2602000000 -7741000000 -937000000 -57000000																																								[+] 8-K (current reports)																																										[+] 10-K (annual reports) and 10-Q (quarterly reports)		Proceeds from Issuance of Long Term Debt																																								[+] Proxy (annual meeting) and information statements																																										[+] Ownership disclosures		Repayments for Long Term Debt 2923000000 -2453000000 -2184000000 -1647000000																																								Filings																																										Search table From Date (yyyy-mm-dd) To Date (yyyy-mm-dd)		Proceeds from Issuance/Exercising of Stock Options/Warrants 00000 300000000 10000000 338000000000																																																																																		Keywords:		Other Financing Cash Flow																																								Show columns:																																										Form type		Cash and Cash Equivalents, End of Period																																								Form description																																										Filing date		Change in Cash 20945000000 23719000000 23630000000 26622000000 26465000000																																								Reporting date																																										Act		Effect of Exchange Rate Changes 25930000000) 235000000000 -3175000000 300000000 6126000000																																								Film number																																										File number		Cash and Cash Equivalents, Beginning of Period PAGE="$USD(181000000000)".XLS BRIN="$USD(146000000000)".XLS 183000000 -143000000 210000000																																								Accession number																																										Size		Cash Flow Supplemental Section 23719000000000 26622000000000 26465000000000 20129000000000																																								"Formtype"																																										"Formtype"		Change in Cash as Reported, Supplemental 2774000000 89000000 -2992000000 6336000000																																								4																																										4		Income Tax Paid, Supplemental 13412000000 157000000																																								4/A																																										4		ZACHRY T WOOD -4990000000																																								4																																										4		Cash and Cash Equivalents, Beginning of Period																																								4																																										4		Department of the Treasury																																								4																																										4		Internal Revenue Service																																								4																																										4		Q4 2020 Q4 2019																																								4																																										4		Calendar Year																																								4																																										4		Due: 04/18/2022																																								4																																										SC 13G/A		Dec. 31, 2020 Dec. 31, 2019																																								4																																										4		USD in "000'"s																																								4																																										4		Repayments for Long Term Debt 182527 161857																																								4																																										4		Costs and expenses:																																								4																																										4		Cost of revenues 84732 71896																																								4																																										4		Research and development 27573 26018																																								4																																										4		Sales and marketing 17946 18464																																								4																																										4		General and administrative 11052 09551																																								5																																										SC 13G/A		European Commission fines 00000 01697																																								5																																										5		Total costs and expenses 141303 127626																																								5																																										5		Income from operations 41224 34231																																								4																																										4		Other income (expense), net 6858000000 05394																																								SC 13G/A																																										SC 13G/A		Income before income taxes 22677000000 19289000000																																								5																																										4		Provision for income taxes 22677000000 19289000000 Net income 22677000000 19289000000																																								4																																										4		*include interest paid, capital obligation, and underweighting																																								4																																										4		Basic net income per share of Class A and B common stock and Class C capital stock (in dollars par share)																																								4																																										4		Diluted net income per share of Class A and Class B common stock and Class C capital stock (in dollars par share) *include interest paid, capital obligation, and underweighting																																								4																																										4		Basic net income per share of Class A and B common stock and Class C capital stock (in dollars par share) Diluted net income per share of Class A and Class B common stock and Class C capital stock (in dollars par share)																																								4																																										4		20210418																																								4																																										4		Rate Units Total YTD Taxes / Deductions Current YTD																																								4																																										4		70842745000 70842745000 Federal Withholding 00000 188813800																																								4		FICA - Social Security 00000 853700 FICA - Medicare 00000 11816700																																								SC 13G/A																																										SC 13G/A		Employer Taxes																																								4																																										4		FUTA 00000 00000																																								4																																										4		SUTA 00000 00000																																								4																																										SC 13G		EIN: 61-1767919 ID : 00037305581 SSN: 633441725 ATAA Payments 00000 102600																																								4																																										4		Gross																																								4																																										4		70842745000 Earnings Statement																																								4																																										4		Taxes / Deductions Stub Number: 1																																								4																																										4		0																																								4																																										4		Net Pay SSN Pay Schedule Pay Period Sep 28, 2022 to Sep 29, 2023 Pay Date 4/18/2022																																								SC 13G/A																																										4		70842745000 XXX-XX-1725 Annually																																								S-3ASR																																										10-K		CHECK NO. 5560149																																								4																																										8-K		INTERNAL REVENUE SERVICE,																																																																																				PO BOX 1214, CHARLOTTE, NC 28201-1214																																																																																		4		ZACHRY WOOD																																								4																																										4		00015 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000 6836000000 10671000000 7068000000																																								4																																										4		For Disclosure, Privacy Act, and Paperwork Reduction Act Notice, see separate instructions. 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000 6836000000 10671000000 7068000000 Cat. No. 11320B 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000 6836000000 10671000000 7068000000																																								4																																										4		Form 1040 (2021) 76033000000 20642000000 18936000000																																								4																																										4		Reported Normalized and Operating Income/Expense Supplemental Section																																								4																																										4		Total Revenue as Reported, Supplemental 257637000000 75325000000 65118000000 61880000000 55314000000 56898000000 46173000000 38297000000 41159000000 46075000000 40499000000																																								4																																										4		Total Operating Profit/Loss as Reported, Supplemental 78714000000 21885000000 21031000000 19361000000 16437000000 15651000000 11213000000 6383000000																																								4																																										4		7977000000 9266000000 9177000000 Reported Effective Tax Rate 00000 00000 00000 00000 00000 00000 00000 00000 00000																																								4																																										4		Reported Normalized Income 6836000000																																								4																																										4		Reported Normalized Operating Profit 7977000000																																								4																																										4		Other Adjustments to Net Income Available to Common Stockholders																																								4																																										4		Discontinued Operations																																								4																																										4		Basic EPS 00114 00031 00028 00028 00027 00023 00017 00010 00010 00015 00010																																								4																																										4		Basic EPS from Continuing Operations 00114 00031 00028 00028 00027 00022 00017 00010 00010 00015 00010																																								4																																										4		Basic EPS from Discontinued Operations																																								8-K																																												Diluted EPS 00112 00031 00028 00027 00026 00022 00016 00010 00010 00015 00010																																								4																																										4		Diluted EPS from Continuing Operations 00112 00031 00028 00027 00026 00022 00016 00010 00010 00015 00010																																								4																																										4		Diluted EPS from Discontinued Operations																																								4																																										4		Basic Weighted Average Shares Outstanding 667650000 662664000 665758000 668958000 673220000 675581000 679449000 681768000 686465000 688804000 692741000																																								4																																										4		Diluted Weighted Average Shares Outstanding 677674000 672493000 676519000 679612000 682071000 682969000 685851000 687024000 692267000 695193000 698199000																																								3																																										4		Reported Normalized Diluted EPS 00010																																								4																																										4		Basic EPS 00114 00031 00028 00028 00027 00023 00017 00010 00010 00015 00010 00001																																								4																																										4		Diluted EPS 00112 00031 00028 00027 00026 00022 00016 00010 00010 00015 00010																																								4																																										4		Basic WASO 667650000 662664000 665758000 668958000 673220000 675581000 679449000 681768000 686465000 688804000 692741000																																								4																																										4		Diluted WASO 677674000 672493000 676519000 679612000 682071000 682969000 685851000 687024000 692267000 695193000 698199000																																								4																																										4		Fiscal year end September 28th., 2022. | USD/30/2022																																								4																																										4		NOTICE UNDER THE PAPERWORK REDUCTION ACT Bureau of the Fiscal Service, Forms Management Officer, Parkersburg, WV 26106-1328.																																								4																																										4		FOR USE BY THE BUREAU OF THE FISCAL SERVICE																																								4																																										4		E'-Customer ID Processed by /FS Form 4144 Department of the Treasury | Bureau of the Fiscal Service Revised August 2018 Form Instructions Bureau of the Fiscal Service Special Investments Branch P.O. Box 396, Room 119 Parkersburg, WV 26102-0396 Telephone Number: (304) 480-5299 Fax Number: (304) 480-5277 Internet Address: https://www.slgs.gov/ E-Mail Address: SLGS@fiscal.treasury.gov Governing Regulations: 31 CFR Part 344 Please add the following information prior to mailing the form: • The name of the organization should be entered in the first paragraph. • If the user does not have an e-mail address, call SIB at 304-480-5299 for more information. • The user should sign and date the form. • If the access administrator or backup administrator also completes a user acknowledgment, both administrators should sign the 4144-5 Application for Internet Access. Regular Mail Address: Courier Service Address: Bureau of the Fiscal Service Special Investments Branch P.O. Box 396, Room 119 Parkersburg, WV 26102-0396 The Special Investments Branch (SIB) will only accept original signatures on this form. SIB will not accept faxed or emailed copies. Tax Periood Requested : December, 2020 Form W-2 Wage and Tax Statement Important Notes on Form 8-K, as filed with the Commission on January 18, 2019).																																								4																																										4		  Request Date : 07-29-2022																																								4		 																																								4		  Period Beginning: 37151																																								4																																										4		  Response Date : 07-29-2022																																								4/A		 																																								4		  Period Ending: 44833																																								4																																										4		  Tracking Number : 102393399156																																								4		 																																								4		  Pay Date: 44591																																								4																																										4		  Customer File Number : 132624428																																								4		 																																								4		  ZACHRY T. WOOD																																								4																																										4		  5323 BRADFORD DR          important information Wage and Income Transcript																																								4		SSN Provided : XXX-XX-1725 DALLAS TX 75235-8314 Submis sion Type : Original document																																								4																																										4		Wages, Tips and Other Compensation : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 5105000.00 510500000																																								4		Advice number: 650001																																								4																																										4		Federal Income Tax Withheld : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 1881380.00 188813800 Pay date: Monday, April 18, 2022																																								4																																										4		Social Security Wages : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 137700.00 13770000																																								4																																										4		Social Security Tax Withheld : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .																																								4																																										4		Social Security Tips : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .00000																																								4																																										10-Q		Allocated Tips : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000																																								4																																										8-K		Dependent Care Benefits : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000																																																																																				Deffered Compensation : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000																																								4																																										4		Code "Q" Nontaxable Combat Pay : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000																																								4																																										4		Code "W" Employer Contributions tp a Health Savings Account : . . . . . . . . . . . . . . . . . . . . . . . . . . 00000																																								4																																										4		Code "Y" Defferels under a section 409A nonqualified Deferred Compensation plan : . . . . . . . . . . . . . . . . . . 00000																																								4																																										4		Code "Z" Income under section 409A on a nonqualified Deferred Compensation plan : . . . . . . . . . . . . . . . . . 00000																																								4																																										4		Code "R" Employer's Contribution to MSA : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .' 00000																																								4																																										4		Code "S" Employer's Cotribution to Simple Account : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000																																								4																																										4		Code "T" Expenses Incurred for Qualified Adoptions : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000																																								4																																										4		Code "V" Income from exercise of non-statutory stock options : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000																																								4																																										4		Code "AA" Designated Roth Contributions under a Section 401 (k) Plan : . . . . . . . . . . . . . . . . . . . . 00000																																								4																																										4		Code "BB" Designated Roth Contributions under a Section 403 (b) Plan : . . . . . . . . . . . . . . . . . . . . . 00000																																								4																																										4		Code "DD" Cost of Employer-Sponsored Health Coverage : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .																																								4																																										4		Code "EE" Designated ROTH Contributions Under a Governmental Section 457 (b) Plan : . . . . . . . . . . . . . . . . . . . . .																																								4																																										4		Federal 941 Deposit Report																																								4																																										4		ADP Report Range 5/4/2022 - 6/4/2022 00519																																								4																																										4		88-1303491 State ID: 00037305581 SSN: 633-44-1725 00000																																								4																																										4		Employee Number: 3																																								3																																										4		Description Amount 5/4/2022 - 6/4/2022																																								4																																										4		Payment Amount (Total) 9246754678763 Display All																																								4																																										4		Social Security (Employee + Employer) 26662																																								4																																										4		Medicare (Employee + Employer) 861193422444 Hourly																																								4																																										4		Federal Income Tax 8385561229657 00000																																								4																																										4		Note: This report is generated based on the payroll data for your reference only. Please contact IRS office for special cases such as late payment, previous overpayment, penalty and others.																																								4																																										4		Note: This report doesn't include the pay back amount of deferred Employee Social Security Tax.																																								4																																										4		Employer Customized Report																																								4																																										4		ADP Report Range5/4/2022 - 6/4/2022 88-1656496 state ID: 633441725 Ssn :XXXXX1725 State: All Local ID: 00037305581 2267700																																								4																																										4		EIN:																																								4																																										4		Customized Report Amount Employee Payment Report																																								4																																										4		ADP																																								4																																										4		Employee Number: 3																																								4																																										4		Description Home > Chapter 7: Reports > Custom Reports > Exporting Custom Reports > Export Custom Report as Excel File																																								4																																										4		Wages, Tips and Other Compensation 22662983361014 Tips																																		Info 				SSN 		4																																										4		Taxable SS Wages 215014 5105000																																								4																																										4		Taxable SS Tips 00000																																								4																																										4		Taxable Medicare Wages 22662983361014 Salary Vacation hourly OT Advanced EIC Payment 00000 3361014																																								4																																										4		Federal Income Tax Withheld 8385561229657 Bonus 00000 00000																																								4																																										4		Employee SS Tax Withheld 13331 00000 Other Wages 1 Other Wages 2																																								4																																										4		Employee Medicare Tax Withheld 532580113436 Total 00000 00000																																								10-Q																																										3		State Income Tax Withheld 00000 22662983361014																																								8-K																																												Local Income Tax Withheld																																																																																		4		Customized Employer Tax Report 00000 Deduction Summary																																								4																																										4		Description Amount Health Insurance																																								4																																										4		Employer SS Tax																																								4																																										4		Employer Medicare Tax 13331 00000																																								4																																										4		Federal Unemployment Tax 328613309009 Tax Summary																																								4																																										4		State Unemployment Tax 00442 Federal Tax 00007 Total Tax																																								4																																										4		Customized Deduction Report 00840 $8,385,561,229,657@3,330.90 Local Tax																																								4																																										4		Health Insurance 00000																																								8-K																																												401K 00000 Advanced EIC Payment 8918141356423																																								4																																										4		00000 00000 Total																																								4																																										4		401K																																								4																																										4		00000 00000																																								4																																										4		ZACHRY T WOOD Social Security Tax Medicare Tax State Tax 532580113050																																								4																																										4		SHAREHOLDERS ARE URGED TO READ THE DEFINITIVE PROXY STATEMENT AND ANY OTHER RELEVANT MATERIALS THAT THE COMPANY WILL FILE WITH THE SEC CAREFULLY IN THEIR ENTIRETY WHEN THEY BECOME AVAILABLE. SUCH DOCUMENTS WILL CONTAIN IMPORTANT INFORMATION ABOUT THE COMPANY AND ITS DIRECTORS, OFFICERS AND AFFILIATES.																																								4																																										4		INFORMATION REGARDING THE INTERESTS OF CERTAIN OF THE COMPANY’S DIRECTORS, OFFICERS AND AFFILIATES WILL BE AVAILABLE IN THE DEFINITIVE PROXY STATEMENT.																																								4																																										4		The Definitive Proxy Statement and any other relevant materials that will be filed with the SEC will be available free of charge at the SEC’s website at www.sec.gov. In addition, the Definitive Proxy Statement (when available) and other relevant documents will also be available, without charge, by directing																																								4																																										4		a request by mail to Attn: Investor Relations, Alphabet Inc., 1600 Amphitheatre Parkway, Mountain View, California, 94043 or by contacting investor- relations@abc.xyz. The Definitive Proxy Statement and other relevant documents will also be available on the Company’s Investor Relations website at https://abc.xyz/investor/other/annual-meeting/. The Company and its directors and certain of its executive officers may be consideredno participants in the solicitation of proxies with respect to the proposals under the Definitive Proxy Statement under the rules of the SEC. Additional information regarding the participants in the proxy solicitations and a description of their direct and indirect interests, by security holdings or otherwise, also will be included in the Definitive Proxy Statement and other relevant materials to be filed with the SEC when they become available.																																								4																																										4		9246754678763 3/6/2022 at 6:37 PM																																								4																																										4		Q4 2021 Q3 2021 Q2 2021 Q1 2021 Q4 2020																																								4																																										4		GOOGL_income-statement_Quarterly_As_Originally_Reported 24934000000 25539000000 37497000000 31211000000 30818000000 4934000000 25539000000 21890000000 19289000000 22677000000																																								Showing 1 to 32 of 1,000 entries																																										Data source: CIK0001652044.json		Cash Flow from Operating Activities, Indirect 24934000000 25539000000 21890000000 19289000000 22677000000																																								Investor Resources																																										How to Use EDGAR		Net Cash Flow from Continuing Operating Activities, Indirect 20642000000 18936000000 18525000000 17930000000 15227000000																																								Learn how to use EDGAR to research public filings by public companies, mutual funds, ETFs, some annuities, and more.																																										Before you Invest, Investor.gov		Cash Generated from Operating Activities 6517000000 3797000000 4236000000 2592000000 5748000000																																								Get answers to your investing questions from the SEC's website dedicated to retail investors
April 11, 2022.
With the Approval of (Mr. Joe Biden) the 46th president of the United States 
The American Rescue Plan Act (TARP/COVID-19 Stimulus Package) program which was enacted by the 117th United States Congress is 
to help relieve the burdens on families across the globe and provide for reconciliation pursuant to title II of S. Con. Res. 5 
in effect that, this notice is sent to let you know that you're among the beneficiaries of this program and you'reentitled to the paycheck 
sum of USD$5.2 million onlyto claim your paycheck, you'll have to provide the details of you as below:
Request :Date :07-29-2022 
Response Date : 07-29-2022 
Tracking Number : 102393399156 
Customer File Number : 132624428 
Wage and Income Transcript
Tax Period Requested : December, 2020

Form W-2 Wage and Tax Statement Important Notes
Employer :
Employer Identification Number (EIN) :XXXXX4661 BASIS OF PAY: BASIC/DILUTED EPS
THE
101 EA

Employee :
Reciepient's Identification Number :xxx-xx-1725
ZACH T WOOD
5222 B
Submission Type : . . . . . . . . . . . . . . . . . . . . . . . . . . Original document
Wages, Tips and Other Compensation : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 5105000.00
Federal Income Tax Withheld : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .1881380.00 
Social Security Wages : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .137700.00 
Social Security Tax Withheld : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 853700 
Medicare Wages and Tips : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .0
Medicare Tax Withheld : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 0
Social Security Tips : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 0
Allocated Tips : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .0
Dependent Care Benefits : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 0
Deffered Compensation : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 0
Code "Q" Nontaxable Combat Pay : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .0

Code "W" Employer Contributions tp a Health Savings Account : . . . . . . . . . . . . . . . . . . . . . . . . . .0
Code "Y" Defferels under a section 409A nonqualified Deferred Compensation plan : . . . . . . . .0
Code "Z" Income under section 409A on a nonqualified Deferred Compensation plan : . . . . . . .0
Code "R" Employer's Contribution to MSA : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .0
Code "S" Employer's Cotribution to Simple Account : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 0
Code "T" Expenses Incurred for Qualified Adoptions : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .0
Code "V" Income from exercise of non-statutory stock options : . . . . . . . . . . . . . . . . . . . . . . . . . . .0
Code "AA" Designated Roth Contributions under a Section 401 (k) Plan : . . . . . . . . . . . . . . . . . . . 0
Code "BB" Designated Roth Contributions under a Section 403 (b) Plan : . . . . . . . . . . . . . . . . . . . 0
Code "DD" Cost of Employer-Sponsored Health Coverage : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .0
Code "EE" Designated ROTH Contributions Under a Governmental Section 457 (b) Plan : . . . . . 0
Code "FF" Permitted benefits under a qualified small employer health reimbursment arrangement : . . . . . . . . . 0
Code "GG" Income from Qualified Equity Grants Under Section 83 (i) : . . . . . . . . . . . . . . . . . . . . . . $0.00
Code "HH" Aggregate Defferals Under section 83(i) Elections as of the Close of the Calendar Year : . . . . . . . 0
Third Party Sick Pay Indicator : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . Unanswered
Retirement Plan Indicator : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . Unanswered
Statutory Employee : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . Not Statutory Employee
W2 Submission Type : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . Original
W2 WHC SSN Validation Code : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . Correct SSN
Federal 941 Deposit Report
ADP
Report Range5/4/2022 - 6/4/2022 Local ID:
EIN: 63-3441725State ID: 633441725

Employee N.A. umboeurn:t3
Description 5/4/2022 - 6/4/2022
Payment Amount (Total) $9,246,754,678,763.00 Display All
1. Social Security (Employee + Employer) $26,661.80
2. Medicare (Employee + Employer) $861,193,422,444.20 Hourly
3. Federal Income Tax $8,385,561,229,657.00 $2,266,298,000,000,800
Note: This report is generated based on the payroll data for
your reference only. Please contact IRS office for special
cases such as late payment, previous overpayment, penalty
and others.
Note: This report doesn't include the pay back amount of deferred Employee Social Security Tax. Commission
Employer Customized Report
ADP
Report Range5/4/2022 - 6/4/2022 88-1656496state ID: 633441725 State: All Local ID: 00037305581 $2,267,70000,0.00
EIN:
Customized Report Amount
Employee Payment Report
ADP

Employee Number: 3
Description
Wages, Tips and Other Compensation $22,662,983,361,013.70 Report Range: Tips
Taxable SS Wages $215,014.49
Name:
SSN: $0.00
Taxable SS Tips $0 Payment Summary
Taxable Medicare Wages $22,662,983,361,013.70 Salary Vacation hourly OT
Advanced EIC Payment $0.00 $3,361,013.70
Federal Income Tax Withheld $8,385,561,229,657 Bonus $0.00 $0.00

Employee SS Tax Withheld $13,330.90 $0.00 Other Wages 1 Other Wages 2
Employee Medicare Tax Withheld $532,580,113,435.53 Total $0.00 $0.00
State Income Tax Withheld $0.00 $22,662,983,361,013.70
Local Income Tax Withheld
Customized Employer Tax Report $0.00 Deduction Summary
Description Amount Health Insurance
Employer SS Tax

Employer Medicare Tax $13,330.90 $0.00
Federal Unemployment Tax $328,613,309,008.67 Tax Summary
State Unemployment Tax $441.70 Federal Tax Total Tax
Customized Deduction Report $840 $8,385,561,229,657@3,330.90 Local Tax
Health Insurance $0.00
401K $0.00 Advanced EIC Payment $8,918,141,356,423.43
$0.00 $0.00 Total
401K
$0.00 $0.00
Social Security Tax Medicare TaxState Tax
$532,580,113,050
--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------3/6/2022 at 6:37 PM
Q4 2021 Q3 2021 Q2 2021 Q1 2021 Q4 2020
GOOGL_income￾statement_Quarterly_As_Originally_Reported 
24,934,000,000 25,539,000,000 37,497,000,000 31,211,000,000 
30,818,000,000 24,934,000,000 25,539,000,000 21,890,000,000 19,289,000,000 22,677,000,000
Cash Flow from Operating Activities, Indirect 24,934,000,000 25,539,000,000 21,890,000,000 19,289,000,000 22,677,000,000
Net Cash Flow from Continuing Operating Activities, Indirect 20,642,000,000 18,936,000,000 18,525,000,000 17,930,000,000 15,227,000,000
Cash Generated from Operating Activities 6,517,000,000 3,797,000,000 4,236,000,000 2,592,000,000 5,748,000,000
Income/Loss before Non-Cash Adjustment 3,439,000,000 3,304,000,000 2,945,000,000 2,753,000,000 3,725,000,000

Total Adjustments for Non-Cash Items 3,439,000,000 3,304,000,000 2,945,000,000 2,753,000,000 3,725,000,000
Depreciation, Amortization and Depletion, Non-Cash
Adjustment 3,215,000,000 3,085,000,000 2,730,000,000 2,525,000,000 3,539,000,000
Depreciation and Amortization, Non-Cash Adjustment 224,000,000 219,000,000 215,000,000 228,000,000 186,000,000
Depreciation, Non-Cash Adjustment 3,954,000,000 3,874,000,000 3,803,000,000 3,745,000,000 3,223,000,000
Amortization, Non-Cash Adjustment 1,616,000,000 -1,287,000,000 379,000,000 1,100,000,000 1,670,000,000
Stock-Based Compensation, Non-Cash Adjustment -2,478,000,000 -2,158,000,000 -2,883,000,000 -4,751,000,000 -3,262,000,000
Taxes, Non-Cash Adjustment -2,478,000,000 -2,158,000,000 -2,883,000,000 -4,751,000,000 -3,262,000,000
Investment Income/Loss, Non-Cash Adjustment -14,000,000 64,000,000 -8,000,000 -255,000,000 392,000,000
Gain/Loss on Financial Instruments, Non-Cash Adjustment -2,225,000,000 2,806,000,000 -871,000,000 -1,233,000,000 1,702,000,000
Other Non-Cash Items -5,819,000,000 -2,409,000,000 -3,661,000,000 2,794,000,000 -5,445,000,000
Changes in Operating Capital -5,819,000,000 -2,409,000,000 -3,661,000,000 2,794,000,000 -5,445,000,000
Change in Trade and Other Receivables -399,000,000 -1,255,000,000 -199,000,000 7,000,000 -738,000,000
Change in Trade/Accounts Receivable 6,994,000,000 3,157,000,000 4,074,000,000 -4,956,000,000 6,938,000,000
Change in Other Current Assets 1,157,000,000 238,000,000 -130,000,000 -982,000,000 963,000,000
Change in Payables and Accrued Expenses 1,157,000,000 238,000,000 -130,000,000 -982,000,000 963,000,000

Change in Trade and Other Payables 5,837,000,000 2,919,000,000 4,204,000,000 -3,974,000,000 5,975,000,000
Change in Trade/Accounts Payable 368,000,000 272,000,000 -3,000,000 137,000,000 207,000,000
Change in Accrued Expenses -3,369,000,000 3,041,000,000 -1,082,000,000 785,000,000 740,000,000
Change in Deferred Assets/Liabilities
Change in Other Operating Capital
-11,016,000,000 -10,050,000,000 -9,074,000,000 -5,383,000,000 -7,281,000,000
Change in Prepayments and Deposits -11,016,000,000 -10,050,000,000 -9,074,000,000 -5,383,000,000 -7,281,000,000
Cash Flow from Investing Activities
Cash Flow from Continuing Investing Activities -6,383,000,000 -6,819,000,000 -5,496,000,000 -5,942,000,000 -5,479,000,000
-6,383,000,000 -6,819,000,000 -5,496,000,000 -5,942,000,000 -5,479,000,000
Purchase/Sale and Disposal of Property, Plant and Equipment,Net
Purchase of Property, Plant and Equipment -385,000,000 -259,000,000 -308,000,000 -1,666,000,000 -370,000,000
Sale and Disposal of Property, Plant and Equipment -385,000,000 -259,000,000 -308,000,000 -1,666,000,000 -370,000,000
Purchase/Sale of Business, Net -4,348,000,000 -3,360,000,000 -3,293,000,000 2,195,000,000 -1,375,000,000
Purchase/Acquisition of Business -40,860,000,000 -35,153,000,000 -24,949,000,000 -37,072,000,000 -36,955,000,000

Purchase/Sale of Investments, Net
Purchase of Investments 36,512,000,000 31,793,000,000 21,656,000,000 39,267,000,000 35,580,000,000
100,000,000 388,000,000 23,000,000 30,000,000 -57,000,000
Sale of Investments
Other Investing Cash Flow -15,254,000,000
Purchase/Sale of Other Non-Current Assets, Net -16,511,000,000 -15,254,000,000 -15,991,000,000 -13,606,000,000 -9,270,000,000Sales of Other Non-Current Assets -16,511,000,000 -12,610,000,000 -15,991,000,000 -13,606,000,000 -9,270,000,000

Cash Flow from Financing Activities -13,473,000,000 -12,610,000,000 -12,796,000,000 -11,395,000,000 -7,904,000,000
Cash Flow from Continuing Financing Activities 13,473,000,000 -12,796,000,000 -11,395,000,000 -7,904,000,000
Issuance of/Payments for Common Stock, Net -42,000,000
Payments for Common Stock 115,000,000 -42,000,000 -1,042,000,000 -37,000,000 -57,000,000
Proceeds from Issuance of Common Stock 115,000,000 6,350,000,000 -1,042,000,000 -37,000,000 -57,000,000
Issuance of/Repayments for Debt, Net 6,250,000,000 -6,392,000,000 6,699,000,000 900,000,000 0
Issuance of/Repayments for Long Term Debt, Net 6,365,000,000 -2,602,000,000 -7,741,000,000 -937,000,000 -57,000,000
Proceeds from Issuance of Long Term Debt
Repayments for Long Term Debt 2,923,000,000 -2,453,000,000 -2,184,000,000 -1,647,000,000
Proceeds from Issuance/Exercising of Stock Options/Warrants 0 300,000,000 10,000,000 3.38E+11
Other Financing Cash Flow
Cash and Cash Equivalents, End of Period
Change in Cash 20,945,000,000 23,719,000,000 23,630,000,000 26,622,000,000 26,465,000,000
Effect of Exchange Rate Changes 25930000000 235000000000 -3,175,000,000 300,000,000 6,126,000,000
Cash and Cash Equivalents, Beginning of Period 181000000000 146000000000183,000,000 -143,000,000 210,000,000
Cash Flow Supplemental Section $23,719,000,000,000.00 $26,622,000,000,000.00 $26,465,000,000,000.00 $20,129,000,000,000.00
Change in Cash as Reported, Supplemental 2,774,000,000 89,000,000 -2,992,000,000 6,336,000,000
Income Tax Paid, Supplemental 13,412,000,000 157,000,000
ZACHRY T WOOD -4990000000
Cash and Cash Equivalents, Beginning of Period
Department of the Treasury
Internal Revenue Service
Q4 2020 Q4 2019
Calendar Year Due: 04/18/2022
Dec. 31, 2020 Dec. 31, 2019
USD in "000'"s
Repayments for Long Term Debt 182527 161857
Costs and expenses:
Cost of revenues 84732 71896
Research and development 27573 26018
Sales and marketing 17946 18464
General and administrative 11052 9551
European Commission fines 0 1697
Total costs and expenses 141303 127626
Income from operations 41224 34231
Other income (expense), net 6858000000 5394
Income before income taxes 22,677,000,000,000 19,289,000,000,000
Provision for income taxes 22,677,000,000,000 19,289,000,000,000
Net Income 22,677,000,000 19,289,000,000
*include interest paid, capital obligation, and underweighting Basic net income per share of Class A and B common stock and Class C 

capital stock (in dollars par share)
Diluted net income per share of Class A and Class B common stock and Class C capital stock (in dollars par share)*include interest paid, capital obligation, and underweighting
Basic net income per share of Class A and B common stockand Class C capital stock (in dollars par share)
Diluted net income per share of Class A and Class B commonstock and Class C capital stock (in dollars par share)
ALPHABET 88-1303491
5323 BRADFORD DR,
DALLAS, TX 75235-8314
Employee Info
Isssuer United States Department of The Treasury
Employee Id 9999999998 IRS No. 000000000000
Remitter INTERNAL REVENUE SERVICE, $20,210,418.00
PO BOX 1214, Rate Units Total YTD Taxes / Deductions Current YTD
CHARLOTTE, NC 28201-1214 - - $70,842,745,000.00 $70,842,745,000.00 Federal Withholding $0.00 $0.00
Earnings FICA - Social Security $0.00 $8,853.60
Commissions FICA - Medicare $0.00 $0.00
Employer Taxes 
FUTA $0.00 $0.00
SUTA $0.00 $0.00
EIN: 61-1767ID91:900037305581 SSN: 633441725
YTD Gross Gross
$70,842,745,000.00 $70,842,745,000.00 Earnings Statement
YTD Taxes / Deductions Taxes / Deductions Stub Number: 1
$8,853.60 $0.00
YTD Net Pay net, pay. SSN Pay Schedule Paid Period Sep 28, 2022 to Sep 29, 2023 15-Apr-22 Pay Day 18-Apr-22
$70,842,736,146.40 $70,842,745,000.00 XXX-XX-1725 Annually Sep 28, 2022 to Sep 29, 2023
CHECK DATE CHECK NUMBER 001000
18-Apr-22 
PAY TO THE : ZACHRY WOOD 
ORDER OF : Office of the 46th President Of The United States. 117th US Congress Seal Of The US Treasury Department, 1769 W.H.W. DC, US 2022 . 
INTERNAL REVENUE SERVICE, PO BOX 1214, CHARLOTTE, NC 28201-1214




CHECK AMOUNT ****$70,842,745,000.00**
Pay** *****ZACHRY.WOOD****************** NON-NEGOTIABLE VOID AFTER 14 DAYS
INTERNAL REVENUE SERVICE,
PO BOX 1214,
CHARLOTTE, NC 28201-1214
ZACHRY WOOD
15 $76,033,000,000.00 20,642,000,000 18,936,000,000 18,525,000,000 17,930,000,000 15,227,000,000 11,247,000,000 6,959,000,000 
6,836,000,000 10,671,000,000 7,068,000,000
For Disclosure, Privacy Act, and Paperwork Reduction Act
Notice, see separate instructions. $76,033,000,000.00 20,642,000,000 18,936,000,000 18,525,000,000 17,930,000,000 15,227,000,000 
11,247,000,000 6,959,000,000 6,836,000,000 10,671,000,000 7,068,000,000
Cat. No. 11320B $76,033,000,000.00 20,642,000,000 18,936,000,000 18,525,000,000 17,930,000,000 15,227,000,000 11,247,000,000 
6,959,000,000 6,836,000,000 10,671,000,000 7,068,000,000
Form 1040 (2021) $76,033,000,000.00 20,642,000,000 18,936,000,000
Reported Normalized and Operating Income/Expense
Supplemental Section
Total Revenue as Reported, Supplemental $257,637,000,000.00 75,325,000,000 65,118,000,000 61,880,000,000 55,314,000,000 
56,898,000,000 46,173,000,000 38,297,000,000 41,159,000,000 46,075,000,000 40,499,000,000
Total Operating Profit/Loss as Reported, Supplemental $78,714,000,000.00 21,885,000,000 21,031,000,000 19,361,000,000 
16,437,000,000 15,651,000,000 11,213,000,000 6,383,000,000 7,977,000,000 9,266,000,000 9,177,000,000
Reported Effective Tax Rate $0.16 0.179 0.157 0.158 0.158 0.159 0.119 0.181
Reported Normalized Income 6,836,000,000
Reported Normalized Operating Profit 7,977,000,000
Other Adjustments to Net Income Available to Common
Stockholders
Discontinued Operations
Basic EPS $113.88 31.15 28.44 27.69 26.63 22.54 16.55 10.21 9.96 15.49 10.2
Basic EPS from Continuing Operations $113.88 31.12 28.44 27.69 26.63 22.46 16.55 10.21 9.96 15.47 10.2
Basic EPS from Discontinued Operations
Diluted EPS $112.20 30.69 27.99 27.26 26.29 22.3 16.4 10.13 9.87 15.35 10.12
Diluted EPS from Continuing Operations $112.20 30.67 27.99 27.26 26.29 22.23 16.4 10.13 9.87 15.33 10.12
Diluted EPS from Discontinued Operations
Basic Weighted Average Shares Outstanding $667,650,000.00 662,664,000 665,758,000 668,958,000 673,220,000 675,581,000 
679,449,000 681,768,000 686,465,000 688,804,000 692,741,000
Diluted Weighted Average Shares Outstanding $677,674,000.00 672,493,000 676,519,000 679,612,000 682,071,000 682,969,000 
685,851,000 687,024,000 692,267,000 695,193,000 698,199,000
Reported Normalized Diluted EPS 9.87
Basic EPS $113.88 31.15 28.44 27.69 26.63 22.54 16.55 10.21 9.96 15.49 10.2 1
Diluted EPS $112.20 30.69 27.99 27.26 26.29 22.3 16.4 10.13 9.87 15.35 10.12
Basic WASO $667,650,000.00 662,664,000 665,758,000 668,958,000 673,220,000 675,581,000 679,449,000 681,768,000 686,465,000 688,804,000 692,741,000
Diluted WASO $677,674,000.00 672,493,000 676,519,000 679,612,000 682,071,000 682,969,000 685,851,000 687,024,000 692,267,000 695,193,000 698,199,000
Fiscal year end September 28th., 2022. | USD
For Paperwork Reduction Act Notice, see the seperate
Instructions.
THIS NOTE IS LEGAL TENDER
TENDER
FOR ALL DEBTS, PUBLIC AND
PRIVATE
Current Value
important information
No opinion is expressed on any matters other than those specifically referred to above.


NB:- The rights to your benefited paycheck becomes null and void adter 14 days of non-compliance from the day of recieving this mail


Office of the 46th President Of The United States.


117th US Congress


Seal Of The US Treasury Department, 1769


Remember always that as a law abiding and great citizen of the United States Of America, you're forever entitled to good and healthy 


living, that's why the US government cares much about your well-being


GOD BLESS THE UNITED STATES OF AMERICA

Office of the 46th President Of The United States.


117th US Congress


Seal Of The US Treasury Department, 1769

W.H.W. DC, US 2022 




1 comment on commit 4ca88ad

15
￼
ZACHRY WOODSep 16, 2022, 3:34 PM (2 days ago)
Conversation opened. 1 unread message. Message sent 1 of 19,542 U.S. Department of the Treasury Financial Stability Oversight Council Update Inbox U.S. Departme
￼
ZACHRY WOOD <zachryiixixiiwood@gmail.com>
2:36 PM (10 minutes ago)
￼
￼
to chasebank2040
￼
STATE AND LOCAL GOVERNMENT SERIES: S000002965
From The Desk Of JpMorgan Chase Bank US,
214 Broadway, New York ,
NY 10038 , United States .
Unclaimed Asset/Assets Reunited,
USA International Remittance Department


Greetings dear beneficiary, how are you doing today, I hope all is well with you. Your email was received and a response to your question. I want you to know that your fund has been here since last week and we have made all the necessary arrangements on how to release your fund.


  NOTE: You have three ways which you can receive your fund and these three ways are BANK TO BANK WIRE TRANSFER, CHECK AND ATM CARD. So it is up to you to make your choice and then we will follow up. Be informed that we do not request for transfer charges or delivery charges but the only fee needed for this transaction to be completed is the sum of $675usd and it should be paid to the Country of Origin for the release of Fund Release Order Certificate and Affidavit Stamp and once this is been obtained from the Fund Origin Country, we will commence on the release of your fund without any more delay and latest in two days time your fund will get to you as we have set up all the needed strategies to enable you receive your fund through any means of your choice.


  Your urgent response is needed on this to enable us know if you are serious on this or not because we have many payment files to work on and we will not like to skip yours.


NOTE: Do disregard any email you get from any impostors or offices claiming to be in possession of your funds, you are hereby advised only to be in contact with me as I have been given strict instructions to work under your care and give you guidelines until you receive your overdue funds. Also, you are to forward any emails you get from impostors directly to me so we could act upon, commence an investigation and give the advice to avoid being ripped off.


In anticipating for your urgent cooperation


Thank you, God Bless America.


Mr Jamie Dimon,
Director Of Foreign Remittance Department.
JPMORGAN CHASE BANK & CO
07/30/2022
NOTICE UNDER THE PAPERWORK REDUCTION ACT 
Bureau of the Fiscal Service, 
Forms Management Officer, 
Parkersburg, WV 26106-1328.
FOR USE BY THE BUREAU OF THE FISCAL SERVICE-Customer ID Processed by/FS Form 4144 Department of the Treasury | Bureau of the Fiscal Service Revised August 2018 
Form Instructions :
Bureau of the Fiscal Service 
Special Investments Branch 
P.O. Box 396, Room 119 
Parkersburg, WV 26102-0396 
Telephone Number: (304) 480-5299 
Fax Number: (304) 480-5277 
Internet Address: https://www.slgs.gov/ 
E-Mail Address: SLGS@fiscal.treasury.gov 
Governing Regulations: 31 CFR Part 344 
Please add the following information prior to mailing the form : 
•The name of the organization should be entered in the first paragraph. 
• If the user does not have an e-mail address, call SIB at 304-480-5299 for more information. 
• The user should sign and date the form. 
• If the access administrator or backup administrator also completes a user acknowledgment, both administrators should sign the 4144-5 Application for Internet Access. 
Regular Mail Address: 
Courier Service Address: 
Bureau of the Fiscal Service 
Special Investments Branch 
P.O. Box 396, Room 119 
Parkersburg, WV 26102-0396 
The Special Investments Branch (SIB) will only accept original signatures on this form, SIB, will not accept faxed or emailed copies. Tax Period Requested : December, 2020 
Form W-2G Wage and Tax Statement 
Important Notes on Form 8-K, as filed with the Commission on January 18, 2019).
Request Date : 07-29-2022  
Period Beginning: 37151 
 Response Date : 07-29-2022   
Period Ending: 44833  
Tracking Number : 102393399156   
Pay Date: 44591  
Customer File Number : 132624428   
important information Wage and Income Transcript
SSN Provided : XXX-XX-1725 
DALLAS TX 75235-8314 
Submission Type : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . Original document
Wages, Tips and Other Compensation : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .5105000.00 510500000
Federal Income Tax Withheld :. . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .1881380.00 188813800
Social Security Wages : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .137700.00 13770000
Social Security Tax Withheld : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 
Social Security Tips : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .00000
Allocated Tips : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000Dependent Care Benefits : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000
Deffered Compensation : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000
Code "Q" Nontaxable Combat Pay : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000
Code "W" Employer Contributions tp a Health Savings Account : . . . . . . . . . . . . . . . . . . . . . . . . . . 00000
Code "Y" Defferels under a section 409A nonqualified Deferred Compensation plan : . . . . . . . . . . . . . . . . . . 00000
Code "Z" Income under section 409A on a nonqualified Deferred Compensation plan : . . . . . . . . . . . . . . . . . 00000
Code "R" Employer's Contribution to MSA : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .' 00000
Code "S"Employer's Cotribution to Simple Account : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000
Code "T" Expenses Incurred for Qualified Adoptions : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000
Code "V" Income from exercise of non-statutory stock options : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 00000Code "AA" Designated Roth Contributions under a Section 401 (k) Plan : . . . . . . . . . . . . . . . . . . . . 00000Code "BB" Designated Roth Contributions under a Section 403 (b) Plan : . . . . . . . . . . . . . . . . . . . . . 00000Code "DD" Cost of Employer-Sponsored Health Coverage : . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .Code "EE" Designated ROTH Contributions Under a Governmental Section 457 (b) Plan : . . . . . . . . . . . . . . . . . . . . .Federal Mon,18-Apr, 2020. 941 Deposit ReportADP Report Range 5/4/2022 - 6/4/2022 0051988-1303491 State ID: 00037305581 SSN: 633-44-1725 00000Employee Number: 3Description :Amount :5/4/2022 - 6/4/2022Payment Amount :(Total) 9246754678763 :Display All :Social Security (Employee + Employer) 26662Medicare (Employee + Employer) 861193422444 HourlyFederal Income Tax 8385561229657 00000Note: This report is generated based on the payroll data for your reference only. Please., Contsct the  Contact The office for special cases such as late payment, previous overpayment, penalty and others.Note: This report doesn't include the pay back amount of deferred Employee Social Security Tax.Employer Customized Report ADP Report Range 5/4/2022 - 6/4/2022 FIN :88-1656496 state ID: 633441725 Ssn :XXXXX1725 State: All Local ID: 00037305581 EIN:Customized Report Amount Employee Payment , Tips and Other Compensation 22662983361014 

TipsTaxable SS Wages 215014 510ZZzJPMorgan 
                                                                                                                                The United States of America Co., inc., L.L.C

2017        2018        2019        2020        2021                                                        
-                                                                                                        
-                                                                                                        
-                                                Best Time to 911                                                         
-                                                                                                        
-                                                                                                        
-                                                                                                        
-                                                                                                        
-                                                                                                        
-                                                                                                         
-                                                                                                        
PAY TO THE ORDER OF:

                 INTERNAL REVENUE SERVICE                                                                                        
                 PO BOX 1214                                                                                        
                 CHARLOTTE NC 28201-1214                        9999999999                                                                

PAYABLE TO:
                                                                                                        
-                633-44-1725                                                                                        
-                ZACHRYTWOOD                                                                                        
-                AMPITHEATRE PARKWAY                                                                                        
-                MOUNTAIN VIEW, Califomia 94043                                                                                        
-                                                                                                        
-                                                                                                        
-                                                                                                        
-                                          FIN        61-1767919                                                                        
-                                                                                                        
-                                          EIN        88-1303491                                                                        
-                                                
End Date                                                        44669                                        
-                                                                        
Department of the Treasury           Calendar Year                Check Date        
-                                                                        
Internal Revenue Service                 Due. (04/18/2022)                        
-                                                                                                        
-                                                                                                        
-                                                                                                        
-                                                                                                        
-                                                                
_______________________________________________________________________________________                                        
-                                                                                                        
-                                                                        
Tax Period         Total        Social Security        Medicare        
-                                                                         
IEIN:                                             88-1656495        TxDL:                                  00037305580        SSN:        
-                                                                                        
ZACHRY T WOOD                                          Period Beginning:
ALPHABET                                                                
Alphabet Inc.                                        $134,839                        
Alphabet Inc. GOOGL, GOOG on Nasdaq                                                                
ALPHABET INCOME                                                                Advice number: 650000
Amortization, Non-Cash Adjustment                3215000000        3085000000        2730000000        2525000000        3539000000                
Ann. Rev. Date        161,857        136,819        110,855        90,272        74,989                        
Based on facts as set forth in.                        6550                                        
Based on: 10-K (filing date: 2020-02-04), 10-K (filing date: 2019-02-05), 10-K (filing date: 2018-02-06), 10-K (filing date: 2017-02-03), 10-K (filing date: 2016-02-11).                                                                
Basic EPS
        113.88        31.15        28.44        27.69        26.63        22.54        16.55        10.21
    Basic EPS113.88        31.15        28.44        27.69        26.63        22.54        16.55        10.21
Basic
 EPS from Continuing Operations        113.88        31.12        28.44        27.69        26.63        22.46        16.55        10.21
Basic
 EPS from Discontinued Operations                                                                
Basic
 net income per share of Class A and B common stock 
  Class C capital stock (in dollars par share)                                                                
Basic WASO
        667650000        662664000        665758000        668958000        673220000        675581000        679449000        681768000
Basic Weighted Average Shares Outstanding
        667650000        662664000        665758000        668958000        673220000        675581000        679449000        681768000                                                              


ALPHABET INC Co.
1600 AMPIHTHEATRE PARKWAY
MOUNTAINE VIEW

_____________________________________________________________________________________________________________________________________                                                                
NC Bank National Association
Northern Ky,
    07364
4/18/2022

COMPANY PH/Y. +1 (650) 253-0000

main.  +1 (903) 697-4300

SIGNATURE

Time Zone:Eastern Central Mountain Pacific                        

Investment Products  • Not FDIC Insured  • No Bank Guarantee  • May Lose Value"  
                                                                Social Security Tax
                                                                State location:
                         ;                                      State of incorporation DELAWARE:
Category :                       Statute                                                                BASIS OF PAY: BASIC/DILUTED EPS
Stock-Based Compensation, Non-Cash Adjustment                224000000        219000000        215000000        228000000        186000000                
"Taxable Marital Status: 
Exemptions/Allowances"                        Married                                        ZACHRY T.
Taxes, Non-Cash Adjustment                3954000000        3874000000        3803000000        3745000000        3223000000                
The U.S. Internal Revenue Code of 1986, as amended, the Treasury Regulations promulgated thereunder, published pronouncements of the Internal Revenue Service, which may be cited or used as precedents, and case law, any of which may be changed at any time with retroactive effect.  No opinion is expressed on any matters other than those specifically referred to above.                                                                
Total Adjustments for Non-Cash Items                20642000000        18936000000        18525000000        17930000000        15227000000                
Total costs and expenses                        11052                        9551                
Total Net Finance Income/Expense        1153000000        261000000        310000000        313000000        269000000        333000000        412000000        420000000
Total Operating Profit/Loss        78714000000        21885000000        21031000000        19361000000        16437000000        15651000000        11213000000        6383000000
Total Operating Profit/Loss as Reported, Supplemental        78714000000        21885000000        21031000000        19361000000        16437000000        15651000000        11213000000        6383000000
Total Revenue as Reported, Supplemental        257637000000        75325000000        65118000000        61880000000        55314000000        56898000000        46173000000        38297000000
Total Revenue as Reported, Supplemental        257637000000        75325000000        65118000000        61880000000        55314000000        56898000000        46173000000        38297000000
TX:                NO State Income Tax                                                
US$ in millions        Dec 31, 2019        Dec 31, 2018        Dec 31, 2017        Dec 31, 2016        Dec 31, 2015                        
USD in "000'"s                                                                
Your federal taxable wages this period are $  25763700000000000.00                                                              
ZACHRY T WOOD                                                                                        
-                                                                                
CP 575A (Rev. 2-2007) 

99999999999              
(IRS USE ONLY)                                               575                WOOD                A                   9999999999                                      SS-4                                    
Earnings Statement                                        
-                                                                                                        
-                                                                         EIN:                                             88-1303491       ID:                       txdl                 00037305581        SSN:                                           633-44-1725       DoB:                                           1994-10-15                                                             
-                                                                                                        
-                                        INTERNAL REVENUE SERVICE PO BOX 1300, CHARLOTTE, North Carolina 29201                                                                
                                                                                                        
-                                                                                                        
-                                                                                                        
-                                                                                                        
-                                                                                                        
-                                                                                                        
Employee Information    
                                                                                                                                          
Pay to the order of    
Taxable Marital Status  :                                                                                                                                                                                                                
-        Exemptions/Allowances :                                                                                                                                                                                                                
-        Federal :                                                                                                                                                                                                                
-        TX :  28        rate        units        this period        year to date        Other Benefits and                         
ZACHRY T                                                                                                                                                
-        Current assets:                                0        Information                        
WOOD                                                                                                                                                
-        Cash and cash equivalents        26465        18498                0        Total Work Hrs                                                                                                                                                                        
-        Marketable securities        110229        101177                0        Important Notes                        
DALLAS                                                                                                                                                
-        Total cash, cash equivalents, and marketable securities        136694        119675                0        
COMPANY PH/Y: 650-253-0000                                                0                                                                                                                        
-        Accounts receivable, net        30930        25326                0        
BASIS OF PAY : BASIC/DILUTED  EPS                                                                                                                                                                        
-        Income taxes receivable, net        454        2166                0                                                                                                                                                                                
-        Inventory        728        999                0                                Pto Balance                                                                                                                                                
-        Other current assets        5490        4412                0                                                                                                                                                                                
-        Total current assets        174296        152578                0                                                                                                                                                                                
-        Non-marketable investments        20703        13078                0        70842743866                                                                                                                                                                        
-        Deferred income taxes        1084        721                0                                                                                                                                                                                
-        Property and equipment, net        84749        73646                0        $70,842,743,866.00                                                                                                                                                                         
-        Operating lease assets        12211        10941                0                                                                                                                                                                                
-        Intangible assets, net        1445        1979                0                                                                                                                                                                                
-        Goodwill        21175        20624                0                        
Advice date :        650001                                                                                                                                                
-        Other non-current assets        3953        2342                0                        Pay date :        4/18/2022                                                                                                                                                
-                                                                                                                             
 PLEASE READ THE IMPORTANT DISCLOSURES BELOW.      
:xxxxxxxxx6547        
Jan 29th., 2022                                                                                                                                                
Paid to the account Of :                                0                                519                                                                                                                                                
Accounts payable        5589        5561                0                               
 NON-NEGOTIABLE              

AMPITHEATRE PARKWAY,                                                                
MOUNTAIN VIEW, California 94043711
       
 Department of the Treasury           Calendar Year                                                        Period Ending        9/29/2021                                                                                                                                        
-        Internal Revenue Service        Due 04/18/2022                2022 Form 1040-ES Payment Voucher 1                                        Pay Day          1/30/2022                                                                                                                                        
-        MOUNTAIN VIEW, C.A., 94043                                                                                                                                                                                                                
-        Phone: 650-253-0000                                                                
PLEASE READ THE IMPORTANT DISCLOSURES BELOW 
Taxable Marital Status  :                                                                                                                                                                                                                
-        Exemptions/Allowances :                                                                                                                                                                                                                
-        Federal :                                                                                                                                                                                                                
-        TX :  28        rate        units        this period        year to date        Other Benefits and                         ZACHRY T                                                                                                                                                
-        Current assets:                                0        Information                        WOOD                                                                                                                                                
-        Cash and cash equivalents        26465        18498                0        Total Work Hrs                                                                                                                                                                        
-        Marketable securities        110229        101177                0        Important Notes                        DALLAS                                                                                                                                                
-        Total cash, cash equivalents, and marketable securities        136694        119675                0        COMPANY PH/Y: 650-253-0000                                                0                                                                                                                        
-        Accounts receivable, net        30930        25326                0        BASIS OF PAY : BASIC/DILUTED  EPS                                                                                                                                                                        
-        Income taxes receivable, net        454        2166                0                                                                                                                                                                                
-        Inventory        728        999                0                                Pto Balance                                                                                                                                                
-        Other current assets        5490        4412                0                                                                                                                                                                                
-        Total current assets        174296        152578                0                                                                                                                                                                                
-        Non-marketable investments        20703        13078                0        70842743866                                                                                                                                                                        
-        Deferred income taxes        1084        721                0                                                                                                                                                                                
-        Property and equipment, net        84749        73646                0        $70,842,743,866.00                                                                                                                                                                         
-        Operating lease assets        12211        10941                0                                                                                                                                                                                
-        Intangible assets, net        1445        1979                0                                                                                                                                                                                
-        Goodwill        21175        20624                0                        Advice date :        650001                                                                                                                                                
-        Other non-current assets        3953        2342                0                        Pay date :        4/18/2022                                                                                                                                                
-        PLEASE READ THE IMPORTANT DISCLOSURES BELOW.        319616        275909                0                        :xxxxxxxxx6547        JAn 29th., 2022                                                                                                                                                
-        Paid to the account Of :                                0                                519                                                                                                                                                
-        Accounts payable        5589        5561                0                                NON-NEGOTIABLE                                                                                                                                                
-        Accrued compensation and benefits        11086        8495                0                                                                                                                                                                                
-        Accrued expenses and other current liabilities        28631        23067                0                                                                                                                                                                                
-        Accrued revenue share        7500        5916                0                                                                                                                                                                                
-        Deferred revenue        2543        1908                0                                                                                                                                                                                
-        Income taxes payable, net        1485        274                0                                                                                                                                                                                
-        Total current liabilities        56834        45221                0                                                                                                                                                                                
-        Long-term debt        13932        4554                0                                                                                                                                                                                
-        Deferred revenue, non-current        481        358                0                                                                                                                                                                                
-        Income taxes payable, non-current        8849        9885                0                                                                                                                                                                                
-        Deferred income taxes        3561        1701                0                                                                                                                                                                                
-                11146        10214                0                                                                                                                                                                                
-        Other long-term liabilities        2269        2534                0                                                                                                                                                                                
-        Total liabilities        97072        74467                0                                                                                                                                                                                
-        Commitments and Contingencies (Note 10)                                  0                                                                                                                                                                                
-        Stockholders’ equity:                                0                                                                                                                                                                                
-        Convertible preferred stock, $0.001 par value per share, 100,000 shares authorized; no shares issued and outstanding        0        0                0                                                                                                                                                                                
-        Class A and Class B common stock, and Class C capital stock and additional paid-in capital, $0.001 par value per share: 15,000,000 shares authorized (Class A 9,000,000, Class B 3,000,000, Class C 3,000,000); 688,335 (Class A 299,828, Class B 46,441, Class C 342,066) and 675,222 (Class A 300,730, Class B 45,843, Class C 328,649) shares issued and outstanding        58510        50552                0                                                                                                                                                                                
-        Accumulated other comprehensive income (loss)        633        -1232                0                                                                                                                                                                                
-        Retained earnings        163401        152122                0                                                                                                                                                                                
-        Total stockholders’ equity        222544        201442                0                                                                                                                                                                                
-        Total liabilities and stockholders’ equity        319616        275909                0                                                                                                                                                                                
-        Convertible preferred stock, par value (in dollars per share)        0.001        0.001                0                                                                                                                                                                                
-        Convertible preferred stock, shares authorized (in shares)        100000000        100000000                0                                                                                                                                                                                
-        Convertible preferred stock, shares issued (in shares)        0        0                0                                                                                                                                                                                
-        Convertible preferred stock, shares outstanding (in shares)        0        0                0                                                                                                                                                                                
-        Schedule II: Valuation and Qualifying Accounts (Details) - Allowance for doubtful accounts and sales credits - USD ($) $ in Millions        12 Months Ended                        0                                                                                                                                                                                
-                Dec. 31, 2020        Dec. 31, 2019        Dec. 31, 2018        0                                                                                                                                                                                
-        SEC Schedule, 12-09, Movement in Valuation Allowances and Reserves [Roll Forward]                                0                                                                                                                                                                                
-        Revenues (Narrative) (Details) - USD ($) $ in Billions        12 Months Ended                        0                                                                                                                                                                                
-                Dec. 31, 2020        Dec. 31, 2019                0                                                                                                                                                                                
-        Revenue from Contract with Customer [Abstract]                                0                                                                                                                                                                                
-        Deferred revenue                2.3                0                                                                                                                                                                                
-        Revenues recognized        1.8                        0                                                                                                                                                                                
-        Transaction price allocated to remaining performance obligations        29.8                        0                                                                                                                                                                                
<td id="gmail-diff-b335630551682c19a781afebcf4d07bf978fb1f8ac04c6bf87428ed5106870f5R127" class="gmail-blob-num gmail-blob-num-addition gmail-js-linkable-line-number gmail-js-code-nav-line-number gmail-js-blob-rnum" style="box-sizing:border-box;padding:0px 10px;width:52px;min-wid                                                        
                                                                
                                                                                            
Cover Page - USD ($) $ in Billions                        12 Months Ended
        Jan. 26, 2021        Jun. 30, 2020        Dec. 31, 2020
Document Information [Line Items]                        
Document Type                        10-K
Document Annual Report                        true
Document Period End Date                        Dec. 31,
                2020
Document Transition Report                        false
Entity File Number                        001-37580
Entity Registrant Name                        Alphabet Inc.
Entity Central Index Key                        0001652044
Current Fiscal Year End Date                        --12-31
Document Fiscal Year Focus                        2020
Document Fiscal Period Focus                        FY
Amendment Flag                        false
Entity Incorporation, State or Country Code                        DE
Entity Tax Identification Number                        61-1767919
Entity Address, Address Line One                        1600 Amphitheatre Parkway
Entity Address, City or Town                        Mountain View
Entity Address, State or Province                        CA
Entity Address, Postal Zip Code                        94043
City Area Code                        650
Local Phone Number                        253-000
Entity Well-known Seasoned Issuer                        Yes
Entity Voluntary Filers                        No
Entity Current Reporting Status                        Yes
Entity Interactive Data Current                        Yes
Entity Filer Category                        Large Accelerated Filer
Entity Small Business                        false
Entity Emerging Growth Company                        false
ICFR Auditor Attestation Flag                        true
Entity Shell Company                        false
Entity Public Float                 $ 849.7         
Documents Incorporated by Reference                        DOCUMENTS INCORPORATED BY REFERENCE Portions of the registrant’s Proxy Statement for the 2021 Annual Meeting of Stockholders are incorporated herein by reference in Part III of this Annual Report on Form 10-K to the extent stated herein. Such proxy statement will be filed with the Securities and Exchange Commission within 120 days of the registrant’s fiscal year ended December 31, 2020.
Class A Common Stock                        
Document Information [Line Items]                        
Title of 12(b) Security                        Class A Common Stock, $0.001 par value
Trading Symbol                        GOOGL
Security Exchange Name                        NASDAQ
Entity Common Stock, Shares Outstanding         300,737,081                 
Class B Common Stock                        
Document Information [Line Items]                        
Entity Common Stock, Shares Outstanding         45,843,112                 
Class C Capital Stock                        
Document Information [Line Items]                        
Title of 12(b) Security                        Class C Capital Stock, $0.001 par value
Trading Symbol                        GOOG
Security Exchange Name                        NASDAQ
Entity Common Stock, Shares Outstanding         327,556,472                 
22677000000                        19289000000                
MOINTAIN VIEW, CA, 94043                                                                
                      2022-04-15                         EIN: 88-1303491                                                                 Pay Period: 2021-09-30
ZACHRY T WOOD                                            FIN :88-1656496
ALPHABET                                                                                                                         Paid Date: 2021-09-30                     
5323 BRADFORD DR                                                                                                                   Pay Day: 2022-04-18
61-1767919                                                                
7370 - Services-Computer Programming, Data Processing, Etc.                                                            
Accession number                                                                
Act                                                  
Adjustment Payment to Class C                                
ZACHRY T WOOD                                          Period Beginning:
ALPHABET                                                                
Alphabet Inc.                                        $134,839                        
Alphabet Inc. GOOGL, GOOG on Nasdaq                                                                
ALPHABET INCOME                                                                Advice number: 650000
Amortization, Non-Cash Adjustment                3215000000        3085000000        2730000000        2525000000        3539000000                
Ann. Rev. Date        161,857        136,819        110,855        90,272        74,989                        
Based on facts as set forth in.                        6550                                        
Based on: 10-K (filing date: 2020-02-04), 10-K (filing date: 2019-02-05), 10-K (filing date: 2018-02-06), 10-K (filing date: 2017-02-03), 10-K (filing date: 2016-02-11).                                                                
Basic EPS
        113.88        31.15        28.44        27.69        26.63        22.54        16.55        10.21
    Basic EPS113.88        31.15        28.44        27.69        26.63        22.54        16.55        10.21
Basic
 EPS from Continuing Operations        113.88        31.12        28.44        27.69        26.63        22.46        16.55        10.21
Basic
 EPS from Discontinued Operations                                                                
Basic
 net income per share of Class A and B common stock 
  Class C capital stock (in dollars par share)                                                                
Basic WASO
        667650000        662664000        665758000        668958000        673220000        675581000        679449000        681768000
Basic Weighted Average Shares Outstanding
        667650000        662664000        665758000        668958000        673220000        675581000        679449000        681768000                                                              

5323 BRADFORD DR
DALLAS, TX 94043-2021 
ZACHRY T WOOD
ALPHABET INC Co.
1600 AMPIHTHEATRE PARKWAY
MOUNTAINE VIEW                                                                
NC Bank National Association
Northern Ky,
    07364
4/18/2022

COMPANY PH/Y. +1 (650) 253-0000

main.  +1 (903) 697-4300

SIGNATURE

Time Zone:Eastern Central Mountain Pacific                        

Investment Products  • Not FDIC Insured  • No Bank Guarantee  • May Lose Value"             
Time Zone: Eastern Central Mountain Pacific                                
Investment Products  • Not FDIC Insured  • No Bank Guarantee  • May Lose Value"                                                                                                                                                                                                                                                                                                     
Taxable Marital Status  :                                                                                                                                                                                                                
-        Exemptions/Allowances :                                                                                                                                                                                                                
-        Federal :                                                                                                                                                                                                                
-        TX :  28        rate        units        this period        year to date        Other Benefits and                         ZACHRY T                                                                                                                                                
-        Current assets:                                0        Information                        WOOD                                                                                                                                                
-        Cash and cash equivalents        26465        18498                0        Total Work Hrs                                                                                                                                                                        
-        Marketable securities        110229        101177                0        Important Notes                        DALLAS                                                                                                                                                
-        Total cash, cash equivalents, and marketable securities        136694        119675                0        COMPANY PH/Y: 650-253-0000                                                0                                                                                                                        
-        Accounts receivable, net        30930        25326                0        BASIS OF PAY : BASIC/DILUTED  EPS                                                                                                                                                                        
-        Income taxes receivable, net        454        2166                0                                                                                                                                                                                
-        Inventory        728        999                0                                Pto Balance                                                                                                                                                
-        Other current assets        5490        4412                0                                                                                                                                                                                
-        Total current assets        174296        152578                0                                                                                                                                                                                
-        Non-marketable investments        20703        13078                0        70842743866                                                                                                                                                                        
-        Deferred income taxes        1084        721                0                                                                                                                                                                                
-        Property and equipment, net        84749        73646                0        $70,842,743,866.00                                                                                                                                                                         
-        Operating lease assets        12211        10941                0                                                                                                                                                                                
-        Intangible assets, net        1445        1979                0                                                                                                                                                                                
-        Goodwill        21175        20624                0                        Advice date :        650001                                                                                                                                                
-        Other non-current assets        3953        2342                0                        Pay date :        4/18/2022                                                                                                                                                
-        PLEASE READ THE IMPORTANT DISCLOSURES BELOW.        319616        275909                0                        :xxxxxxxxx6547        JAn 29th., 2022                                                                                                                                                
-        Paid to the account Of :                                0                                519                                                                                                                                                
-        Accounts payable        5589        5561                0                                NON-NEGOTIABLE                                                                                                                                                
-        Accrued compensation and benefits        11086        8495                0                                                                                                                                                                                
-        Accrued expenses and other current liabilities        28631        23067                0                                                                                                                                                                                
-        Accrued revenue share        7500        5916                0                                                                                                                                                                                
-        Deferred revenue        2543        1908                0                                                                                                                                                                                
-        Income taxes payable, net        1485        274                0                                                                                                                                                                                
-        Total current liabilities        56834        45221                0                                                                                                                                                                                
-        Long-term debt        13932        4554                0                                                                                                                                                                                
-        Deferred revenue, non-current        481        358                0                                                                                                                                                                                
-        Income taxes payable, non-current        8849        9885                0                                                                                                                                                                                
-        Deferred income taxes        3561        1701                0                                                                                                                                                                                
-                11146        10214                0                                                                                                                                                                                
-        Other long-term liabilities        2269        2534                0                                                                                                                                                                                
-        Total liabilities        97072        74467                0                                                                                                                                                                                
-        Commitments and Contingencies (Note 10)                                  0                                                                                                                                                                                
-        Stockholders’ equity:                                0                                                                                                                                                                                
-        Convertible preferred stock, $0.001 par value per share, 100,000 shares authorized; no shares issued and outstanding        0        0                0                                                                                                                                                                                
-        Class A and Class B common stock, and Class C capital stock and additional paid-in capital, $0.001 par value per share: 15,000,000 shares authorized (Class A 9,000,000, Class B 3,000,000, Class C 3,000,000); 688,335 (Class A 299,828, Class B 46,441, Class C 342,066) and 675,222 (Class A 300,730, Class B 45,843, Class C 328,649) shares issued and outstanding        58510        50552                0                                                                                                                                                                                
-        Accumulated other comprehensive income (loss)        633        -1232                0                                                                                                                                                                                
-        Retained earnings        163401        152122                0                                                                                                                                                                                
-        Total stockholders’ equity        222544        201442                0                                                                                                                                                                                
-        Total liabilities and stockholders’ equity        319616        275909                0                                                                                                                                                                                
-        Convertible preferred stock, par value (in dollars per share)        0.001        0.001                0                                                                                                                                                                                
-        Convertible preferred stock, shares authorized (in shares)        100000000        100000000                0                                                                                                                                                                                
-        Convertible preferred stock, shares issued (in shares)        0        0                0                                                                                                                                                                                
-        Convertible preferred stock, shares outstanding (in shares)        0        0                0                                                                                                                                                                                
-        Schedule II: Valuation and Qualifying Accounts (Details) - Allowance for doubtful accounts and sales credits - USD ($) $ in Millions        12 Months Ended                        0                                                                                                                                                                                
-                Dec. 31, 2020        Dec. 31, 2019        Dec. 31, 2018        0                                                                                                                                                                                
-        SEC Schedule, 12-09, Movement in Valuation Allowances and Reserves [Roll Forward]                                0                                                                                                                                                                                
-        Revenues (Narrative) (Details) - USD ($) $ in Billions        12 Months Ended                        0                                                                                                                                                                                
-                Dec. 31, 2020        Dec. 31, 2019                0                                                                                                                                                                                
-        Revenue from Contract with Customer [Abstract]                                0                                                                                                                                                                                
-        Deferred revenue                2.3                0                                                                                                                                                                                
-        Revenues recognized        1.8                        0                                                                                                                                                                                
-        Transaction price allocated to remaining performance obligations        29.8                        0                                                                                                                                                                                
<td id="gmail-diff-b335630551682c19a781afebcf4d07bf978fb1f8ac04c6bf87428ed5106870f5R127" class="gmail-blob-num gmail-blob-num-addition gmail-js-linkable-line-number gmail-js-code-nav-line-number gmail-js-blob-rnum" style="box-sizing:border-box;padding:0px 10px;width:52px;min-wid                                                        
                                                                                                                  
Pretax Income
        90734000000        24402000000        23064000000        21985000000        21283000000        18689000000        13359000000        8277000000
Proceeds from Issuance of Common Stock
                13473000000        -12610000000        -12796000000        -11395000000        -7904000000                
Proceeds from Issuance of Long Term Debt
                115000000        -42000000        -1042000000        -37000000        -57000000                
Proceeds from Issuance/Exercising of Stock Options/Warrants
                6365000000        -6392000000        -7741000000        -937000000        -57000000                
Provision for Income Tax
        -14701000000        -3760000000        -4128000000        -3460000000        -3353000000        -3462000000        -2112000000        -1318000000
Provision for income taxes
        36,355        -32,669        25,611        22,198        18,030                        
Provision for income taxes
                        6858000000                        5394                
Purchase of Investments
                -4348000000        -259000000        -3293000000        2195000000        -1375000000                
Purchase of Property, Plant and Equipment
                -6383000000        -10050000000        -5496000000        -5942000000        -5479000000                
Purchase/Acquisition of Business
                -385000000                -308000000        -1666000000        -370000000                
Purchase/Sale and Disposal of Property, Plant and Equipment, Net
                -11016000000        -10050000000        -9074000000        -5383000000        -7281000000                
Purchase/Sale of Business, Net
                        -6819000000                                        
Purchase/Sale of Investments, Net
                -385000000        -259000000        -308000000        -1666000000        -370000000                
Purchase/Sale of Other Non-Current Assets, Net
                    100000000        31793000000        23000000        30000000        -57000000                
Repayments for Long Term Debt
                6250000000        6350000000        6699000000        900000000        0                
Repayments for Long Term Debt                        
Dec. 31, 2020                        Dec. 31, 2019                
Reported Effective Tax Rate
0.162                0.179        0.157        0.158                0.158        0.159
Reported Normalized and Operating Income/Expense Supplemental Section                                                              
Reported Normalized Diluted EPS                                                                
Reported Normalized Income                                                                
Reported Normalized Operating Profit                                                                
Reporting date                                                                
Research and development        
                  -18,464              -16,333                -12,893                -10,485                -9,047                        
Research and development                                                                
Research and Development Expenses
                 -31562000000        -8708000000        -7694000000         -7675000000         -7485000000        -7022000000        -6856000000        -6875000000
Revenues
                 -71,896                -59,549               -45,583               -35,138               -28,164                        
S-3ASR
                Automatic shelf registration statement of securities of well-known seasoned issuersOpen document FilingOpen filing                44594                                
Sale and Disposal of Property, Plant and Equipment 
                -6383000000         -6819000000         -5496000000         -5942000000         -5479000000                
Sale of Investments
                -40860000000        -3360000000        -24949000000       -37072000000        -36955000000                
Sales and marketing
                -9,551                   -8,126                -6,872                -6,985                 -6,136                        
Sales and marketing        
                        84732
                        71896                
Sales of Other Non-Current Assets
         3688000000                                        
SC 13G                
Statement of acquisition of beneficial ownership                                                                 
Selling and Marketing Expenses                     -22912000000        -7604000000        -5516000000        -5276000000        -4516000000        -5314000000        -4231000000        -3901000000
Selling, General and Administrative Expenses        -36422000000        -11744000000        -8772000000        -8617000000        -7289000000        -8145000000        -6987000000        -6486000000
Show columns:                                                                
Showing 1 to 32 of 1,000 entries                                                                
SIC:                                                                
Size                                                                
Social Security Tax                                                                
State location:                                                                
State of incorporation:                                                                
Statutory                                                                BASIS OF PAY: BASIC/DILUTED EPS
Stock-Based Compensation, Non-Cash Adjustment                224000000        219000000        215000000        228000000        186000000                
"Taxable Marital Status: 
Exemptions/Allowances"                        Married                                        ZACHRY T.
Taxes, Non-Cash Adjustment                3954000000        3874000000        3803000000        3745000000        3223000000                
The U.S. Internal Revenue Code of 1986, as amended, the Treasury Regulations promulgated thereunder, published pronouncements of the Internal Revenue Service, which may be cited or used as precedents, and case law, any of which may be changed at any time with retroactive effect.  No opinion is expressed on any matters other than those specifically referred to above.                                                                
Total Adjustments for Non-Cash Items                20642000000        18936000000        18525000000        17930000000        15227000000                
Total costs and expenses                        11052                        9551                
Total Net Finance Income/Expense        1153000000        261000000        310000000        313000000        269000000        333000000        412000000        420000000
Total Operating Profit/Loss        78714000000        21885000000        21031000000        19361000000        16437000000        15651000000        11213000000        6383000000
Total Operating Profit/Loss as Reported, Supplemental        78714000000        21885000000        21031000000        19361000000        16437000000        15651000000        11213000000        6383000000
Total Revenue as Reported, Supplemental        257637000000        75325000000        65118000000        61880000000        55314000000        56898000000        46173000000        38297000000
Total Revenue as Reported, Supplemental        257637000000        75325000000        65118000000        61880000000        55314000000        56898000000        46173000000        38297000000
TX:                NO State Income Tax                                                
US$ in millions        Dec 31, 2019        Dec 31, 2018        Dec 31, 2017        Dec 31, 2016        Dec 31, 2015                        
USD in "000'"s                                                                
Your federal taxable wages this period are $  25763700000000000.00                                                              
ZACHRY T WOOD                                                                
Zachry Tyler Wood  
      
7711        Department of the Treasury        Calendar Year                                                        Period Ending        9/29/2021                                                                                                                                        
-        Internal Revenue Service        Due 04/18/2022                2022 Form 1040-ES Payment Voucher 1                                        Pay Day        1/30/2022                                                                                                                                        
-        MOUNTAIN VIEW, C.A., 94043                                                                                                                                                                                                                
-        Taxable Marital Status  :                                                                                                                                                                                                                
-        Exemptions/Allowances :                                                                                                                                                                                                                
-        Federal :                                                                                                                                                                                                                
-        TX :  28        rate        units        this period        year to date        Other Benefits and                         ZACHRY T                                                                                                                                                
-        Current assets:                                0        Information                        WOOD                                                                                                                                                
-        Cash and cash equivalents        26465        18498                0        Total Work Hrs                                                                                                                                                                        
-        Marketable securities        110229        101177                0        Important Notes                        DALLAS                                                                                                                                                
-        Total cash, cash equivalents, and marketable securities        136694        119675                0        COMPANY PH/Y: 650-253-0000                                                0                                                                                                                        
-        Accounts receivable, net        30930        25326                0        BASIS OF PAY : BASIC/DILUTED  EPS                                                                                                                                                                        
-        Income taxes receivable, net        454        2166                0                                                                                                                                                                                
-        Inventory        728        999                0                                Pto Balance                                                                                                                                                
-        Other current assets        5490        4412                0                                                                                                                                                                                
-        Total current assets        174296        152578                0                                                                                                                                                                                
-        Non-marketable investments        20703        13078                0        70842743866                                                                                                                                                                        
-        Deferred income taxes        1084        721                0                                                                                                                                                                                
-        Property and equipment, net        84749        73646                0        $70,842,743,866.00                                                                                                                                                                         
-        Operating lease assets        12211        10941                0                                                                                                                                                                                
-        Intangible assets, net        1445        1979                0                                                                                                                                                                                
-        Goodwill        21175        20624                0                        Advice date :        650001                                                                                                                                                
-        Other non-current assets        3953        2342                0                        Pay date :        4/18/2022                                                                                                                                                
-        PLEASE READ THE IMPORTANT DISCLOSURES BELOW.        319616        275909                0                        :xxxxxxxxx6547        JAn 29th., Social Security Tax                                                                
State location:                                                                
State of incorporation:                                                                
Statutory                                                                BASIS OF PAY: BASIC/DILUTED EPS
Stock-Based Compensation, Non-Cash Adjustment                224000000        219000000        215000000        228000000        186000000                
"Taxable Marital Status: 
Exemptions/Allowances"                        Married                                        ZACHRY T.
Taxes, Non-Cash Adjustment                3954000000        3874000000        3803000000        3745000000        3223000000                
The U.S. Internal Revenue Code of 1986, as amended, the Treasury Regulations promulgated thereunder, published pronouncements of the Internal Revenue Service, which may be cited or used as precedents, and case law, any of which may be changed at any time with retroactive effect.  No opinion is expressed on any matters other than those specifically referred to above.                                                                
Total Adjustments for Non-Cash Items                20642000000        18936000000        18525000000        17930000000        15227000000                
Total costs and expenses                        11052                        9551                
Total Net Finance Income/Expense        1153000000        261000000        310000000        313000000        269000000        333000000        412000000        420000000
Total Operating Profit/Loss        78714000000        21885000000        21031000000        19361000000        16437000000        15651000000        11213000000        6383000000
Total Operating Profit/Loss as Reported, Supplemental        78714000000        21885000000        21031000000        19361000000        16437000000        15651000000        11213000000        6383000000
Total Revenue as Reported, Supplemental        257637000000        75325000000        65118000000        61880000000        55314000000        56898000000        46173000000        38297000000
Total Revenue as Reported, Supplemental        257637000000        75325000000        65118000000        61880000000        55314000000        56898000000        46173000000        38297000000
TX:                NO State Income Tax                                                
US$ in millions        Dec 31, 2019        Dec 31, 2018        Dec 31, 2017        Dec 31, 2016        Dec 31, 2015                        
USD in "000'"s                                                                
Your federal taxable wages this period are $  25763700000000000.00                                                              
ZACHRY T WOOD                                                                
Zachry Tyler Wood  
2022                                                                                                                                                
-        Paid to the account Of :                                0                                519                                                                                                                                                
-        Accounts payable        5589        5561                0                                NON-NEGOTIABLE                                                                                                                                                
-        Accrued compensation and benefits        11086        8495                0                                                                                                                                                                                
-        Accrued expenses and other current liabilities        28631        23067                0                                                                                                                                                                                
-        Accrued revenue share        7500        5916                0                                                                                                                                                                                
-        Deferred revenue        2543        1908                0                                                                                                                                                                                
-        Income taxes payable, net        1485        274                0                                                                                                                                                                                
-        Total current liabilities        56834        45221                0                                                                                                                                                                                
-        Long-term debt        13932        4554                0                                                                                                                                                                                
-        Deferred revenue, non-current        481        358                0                                                                                                                                                                                
-        Income taxes payable, non-current        8849        9885                0                                                                                                                                                                                
-        Deferred income taxes        3561        1701                0                                                                                                                                                                                
-                11146        10214                0                                                                                                                                                                                
-        Other long-term liabilities        2269        2534                0                                                                                                                                                                                
-        Total liabilities        97072        74467                0                                                                                                                                                                                
-        Commitments and Contingencies (Note 10)                                  0                                                                                                                                                                                
-        Stockholders’ equity:                                0                                                                                                                                                                                
-        Convertible preferred stock, $0.001 par value per share, 100,000 shares authorized; no shares issued and outstanding        0        0                0                                                                                                                                                                                
-        Class A and Class B common stock, and Class C capital stock and additional paid-in capital, $0.001 par value per share: 15,000,000 shares authorized (Class A 9,000,000, Class B 3,000,000, Class C 3,000,000); 688,335 (Class A 299,828, Class B 46,441, Class C 342,066) and 675,222 (Class A 300,730, Class B 45,843, Class C 328,649) shares issued and outstanding        58510        50552                0                                                                                                                                                                                
-        Accumulated other comprehensive income (loss)        633        -1232                0                                                                                                                                                                                
-        Retained earnings        163401        152122                0                                                                                                                                                                                
-        Total stockholders’ equity        222544        201442                0                                                                                                                                                                                
-        Total liabilities and stockholders’ equity        319616        275909                0                                                                                                                                                                                
-        Convertible preferred stock, par value (in dollars per share)        0.001        0.001                0                                                                                                                                                                                
-        Convertible preferred stock, shares authorized (in shares)        100000000        100000000                0                                                                                                                                                                                
-        Convertible preferred stock, shares issued (in shares)        0        0                0                                                                                                                                                                                
-        Convertible preferred stock, shares outstanding (in shares)        0        0                0                                                                                                                                                                                
-        Schedule II: Valuation and Qualifying Accounts (Details) - Allowance for doubtful accounts and sales credits - USD ($) $ in Millions        12 Months Ended                        0                                                                                                                                                                                
-                Dec. 31, 2020        Dec. 31, 2019        Dec. 31, 2018        0                                                                                                                                                                                
-        SEC Schedule, 12-09, Movement in Valuation Allowances and Reserves [Roll Forward]                                0                                                                                                                                                                                
-        Revenues (Narrative) (Details) - USD ($) $ in Billions        12 Months Ended                        0                                                                                                                                                                                
-                Dec. 31, 2020        Dec. 31, 2019                0                                                                                                                                                                                
-        Revenue from Contract with Customer [Abstract]                                0                                                                                                                                                                                
-        Deferred revenue                2.3                0                                                                                                                                                                                
-        Revenues recognized        1.8                        0                                                                                                                                                                                
-        Transaction price allocated to remaining performance obligations        29.8                        0                                                       
                                                                
                                                                	Get answers to your investing questions from the SEC's website dedicated to retail investors																																											
							2			Earnings Statement																																		
																																												
	ALPHABET		        							Period Beginning:																																		
	1601 AMPITHEATRE PARKWAY			DR						Period Ending:	DR																																	
	MOUNTAIN VIEW, C.A., 94044									Pay Date:																																		
	Taxable Marital Status: 
Exemptions/Allowances			Married						ZACHRY T. 	Married																																	
										5324																																		
	Federal:																																											
										DALLAS																																		
	TX:		NO State Incorne Tax																																									
Earnings		rate	units					year to date	Earnings	Other Benefits and																																		
Regular		1349355887.8	2024033775.6					75698871601	Regular	Information																																		
Overtime								        	Overtime	Pto Balance																																		
Bonus
Training								        	Bonus
Training	Total Work Hrs																																		
	Gross Pay	75698871601						        		Important Notes																																		
										COMPANY PH Y: 650-253-0001																																		
Deductions	Statutory								Deductions	BASIS OF PAY: BASIC/DILUTED EPS																																		
	Federal Income Tax				        			        																																				
	Social Security Tax				        			        																																				
										YOUR BASIC/DILUTED EPS RATE HAS BEEN CHANGED FROM 0.001 TO 112.20 PAR SHARE VALUE																																		
	Medicare Tax				        			        																																				
										        																																		
	Net Pay		70842743867		70842743867																																							
	CHECKING				        																																							
	Net Check		70842743867		        																																							
	Your federal taxable wages this period are $																																											
	ALPHABET INCOME									Advice number:																																		
	1601 AMPIHTHEATRE  PARKWAY MOUNTAIN VIEW CA 94043									Pay date:_																																		
																																												
	Deposited to the account Of									xxxxxxxx6548																																		
	PLEASE READ THE IMPORTANT DISCLOSURES BELOW																																	
																																	
FEDERAL RESERVE MASTER SUPPLIER ACCOUNT					31000053-052101023																								COD				
					633-44-1725																				Zachryiixixiiiwood@gmail.com				47-2041-6547		111000614		31000053
PNC Bank																													PNC Bank Business Tax I.D. Number: 633441725				
CIF Department (Online Banking)																													Checking Account: 47-2041-6547				
P7-PFSC-04-F																													Business Type: Sole Proprietorship/Partnership Corporation				
500 First Avenue																													ALPHABET				
Pittsburgh, PA 15219-3128																													5323 BRADFORD DR				
NON-NEGOTIABLE																													DALLAS TX 75235 8313				
																													ZACHRY, TYLER, WOOD				
																										4/18/2022			650-2530-000 469-697-4300				
														SIGNATURE															Time Zone: Eastern Central Mountain Pacific				
Investment Products  • Not FDIC Insured  • No Bank Guarantee  • May Lose Value																																											
										NON-NEGOTIABLE																																		
	PLEASE READ THE IMPORTANT DISCLOSURES BELOW																																											
INTERNAL REVENUE SERVICE, - PO BOX 1214, - CHARLOTTE, NC 28201-1214 - - YTD Gross Gross - 70842745000 70842745000 - YTD Taxes / Deductions Taxes / Deductions - 8853.6 0 - YTD Net Pay Net Pay - 70842736146 70842745000 - CHECK DATE CHECK NUMBER - - - - 0.455555556 - - - Cash and Cash Equivalents, Beginning of Period - -4990000000 - - 12 Months Ended - _________________________________________________________ - Q4 2020 Q4 2019 - Income Statement - USD in "000'"s - Repayments for Long Term Debt Dec. 31, 2020 Dec. 31, 2019 - Costs and expenses: - Cost of revenues 182527 161857 - Research and development - Sales and marketing 84732 71896 - General and administrative 27573 26018 - European Commission fines 17946 18464 - Total costs and expenses 11052 9551 - Income from operations 0 1697 - Other income (expense), net 127626 - Income before income taxes 34231 - Provision for income taxes ID SSN Pay Schedule Pay Period Pay Date 5394 - Net income Employee Info 9999999999 XXX-XX-1725 Annually Sep 28, 2022 to Sep 29, 2023 44669 19289000000 - *include interest paid, capital obligation, and underweighting United States Department of The Treasury 19289000000 - 19289000000 - Earnings Rate Units Total YTD - Commissions Earnings Statement - 41224 Stub Number: 1 - - - - Diluted net income per share of Class A and Class B common stock and Class C capital stock (in dollars par share) - INTERNAL REVENUE SERVICE, *include interest paid, capital obligation, and underweighting 6858000000 - PO BOX 1214, Basic net income per share of Class A and B common stock and Class C capital stock (in dollars par share) 22677000000 - CHARLOTTE, NC 28201-1214 Diluted net income per share of Class A and Class B common stock and Class C capital stock (in dollars par share) 22677000000 - Basic net income per share of Class A and B common stock and Class C capital stock (in dollars par share) 22677000000 - Taxes / Deductions Current YTD - Fiscal year ends in Dec 31 | USD - Rate - - Total - 7567263607 DoB: 1994-10-15 - YTD - - April 18, 2022. - 7567263607 - WOOD ZACHRY Tax Period Total Social Security Medicare Withholding - Fed 941 Corporate 39355 66986.66 28841.48 6745.18 31400 - Fed 941 West Subsidiary 39355 17115.41 7369.14 1723.42 8022.85 - Fed 941 South Subsidiary 39355 23906.09 10292.9 2407.21 11205.98 - Fed 941 East Subsidiary 39355 11247.64 4842.74 1132.57 5272.33 - Fed 941 Corp - Penalty 39355 27198.5 11710.47 2738.73 12749.3 - Fed 940 Annual Unemp - Corp 39355 17028.05 

Gmail	ZACHRY WOOD <zachryiixixiiwood@gmail.com>
(no subject)
ZACHRY WOOD <zachryiixixiiwood@gmail.com>	Fri, Nov 11, 2022 at 10:40 PM
To: Carolyn Robbins <ckrobbins70@gmail.com>
C&E 1049 Department of the Treasury --- Internal Revenue Service (99) OMB No.  1545-0074 IRS Use Only --- Do not write or staple in this space
1040 U.S. Individual Income Tax Return 1 Earnings Statement

ALPHABET         Period Beginning:2019-09-28
1600 AMPITHEATRE PARKWAY DR Period Ending: 2021-09-29
MOUNTAIN VIEW, C.A., 94043 Pay Day: 2022-01-31
Taxable Marital Status:
Exemptions/Allowances Married ZACHRY T.
5323
Federal:
DALLAS
TX: NO State Income Tax
rate units year to date Other Benefits and
EPS 112.2 674678000 75698871600 Information
        Pto Balance
        Total Work Hrs
Gross Pay 75698871600         Important Notes
COMPANY PH Y: 650-253-0000
Statutory BASIS OF PAY: BASIC/DILUTED EPS
Federal Income Tax                
Social Security Tax                
YOUR BASIC/DILUTED EPS RATE HAS BEEN CHANGED FROM 0.001 TO 112.20 PAR SHARE VALUE
Medicare Tax                
       
Net Pay 70842743866 70842743866
CHECKING        
Net Check 70842743866        
Your federal taxable wages this period are $
ALPHABET INCOME CHECK NO.
1600 AMPIHTHEATRE  PARKWAY MOUNTAIN VIEW CA 94043 222129
DEPOSIT TICKET
Deposited to the account Of xxxxxxxx6547
Deposits and Other Additions                                                                                           Checks and Other Deductions Amount
Description Description I Items 5.41
ACH Additions Debit Card Purchases 1 15.19
POS Purchases 2 2,269,894.11
ACH Deductions 5 82
Service Charges and Fees 3 5.2
Other Deductions 1 2,270,001.91
Total Total 12


Daily Balance

Date Ledger balance Date Ledger balance Date Ledger balance
7/30 107.8 8/3 2,267,621.92- 8/8 41.2
8/1 78.08 8/4 42.08 8/10 2150.19-





Daily Balance continued on next page
Date
8/3 2,267,700.00 ACH Web Usataxpymt IRS 240461564036618 (0.00022214903782823)
8/8 Corporate ACH Acctverify Roll By ADP (00022217906234115)
8/10 ACH Web Businessform Deluxeforbusiness 5072270 (00022222905832355)
8/11 Corporate Ach Veryifyqbw Intuit (00022222909296656)
8/12 Corporate Ach Veryifyqbw Intuit (00022223912710109)


Service Charges and Fees
Reference
Date posted number
8/1 10 Service Charge Period Ending 07/29.2022
8/4 36 Returned ItemFee (nsf) (00022214903782823)
8/11 36 Returned ItemFee (nsf) (00022222905832355)







INCOME STATEMENT

INASDAQ:GOOG TTM Q4 2021 Q3 2021 Q2 2021 Q1 2021 Q4 2020 Q3 2020 Q2 2020

Gross Profit 1.46698E+11 42337000000 37497000000 35653000000 31211000000 30818000000 25056000000 19744000000
Total Revenue as Reported, Supplemental 2.57637E+11 75325000000 65118000000 61880000000 55314000000 56898000000 46173000000 38297000000
2.57637E+11 75325000000 65118000000 61880000000 55314000000 56898000000 46173000000 38297000000
Other Revenue
Cost of Revenue -1.10939E+11 -32988000000 -27621000000 -26227000000 -24103000000 -26080000000 -21117000000 -18553000000
Cost of Goods and Services -1.10939E+11 -32988000000 -27621000000 -26227000000 -24103000000 -26080000000 -21117000000 -18553000000
Operating Income/Expenses -67984000000 -20452000000 -16466000000 -16292000000 -14774000000 -15167000000 -13843000000 -13361000000
Selling, General and Administrative Expenses -36422000000 -11744000000 -8772000000 -8617000000 -7289000000 -8145000000 -6987000000 -6486000000
General and Administrative Expenses -13510000000 -4140000000 -3256000000 -3341000000 -2773000000 -2831000000 -2756000000 -2585000000
Selling and Marketing Expenses -22912000000 -7604000000 -5516000000 -5276000000 -4516000000 -5314000000 -4231000000 -3901000000
Research and Development Expenses -31562000000 -8708000000 -7694000000 -7675000000 -7485000000 -7022000000 -6856000000 -6875000000
Total Operating Profit/Loss 78714000000 21885000000 21031000000 19361000000 16437000000 15651000000 11213000000 6383000000
Non-Operating Income/Expenses, Total 12020000000 2517000000 2033000000 2624000000 4846000000 3038000000 2146000000 1894000000
Total Net Finance Income/Expense 1153000000 261000000 310000000 313000000 269000000 333000000 412000000 420000000
Net Interest Income/Expense 1153000000 261000000 310000000 313000000 269000000 333000000 412000000 420000000

Interest Expense Net of Capitalized Interest -346000000 -117000000 -77000000 -76000000 -76000000 -53000000 -48000000 -13000000
Interest Income 1499000000 378000000 387000000 389000000 345000000 386000000 460000000 433000000
Net Investment Income 12364000000 2364000000 2207000000 2924000000 4869000000 3530000000 1957000000 1696000000
Gain/Loss on Investments and Other Financial Instruments 12270000000 2478000000 2158000000 2883000000 4751000000 3262000000 2015000000 1842000000
Income from Associates, Joint Ventures and Other Participating Interests 334000000 49000000 188000000 92000000 5000000 355000000 26000000 -54000000
Gain/Loss on Foreign Exchange -240000000 -163000000 -139000000 -51000000 113000000 -87000000 -84000000 -92000000
Irregular Income/Expenses 0 0 0 0 0
Other Irregular Income/Expenses 0 0 0 0 0
Other Income/Expense, Non-Operating -1497000000 -108000000 -484000000 -613000000 -292000000 -825000000 -223000000 -222000000
Pretax Income 90734000000 24402000000 23064000000 21985000000 21283000000 18689000000 13359000000 8277000000
Provision for Income Tax -14701000000 -3760000000 -4128000000 -3460000000 -3353000000 -3462000000 -2112000000 -1318000000
Net Income from Continuing Operations 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000
Net Income after Extraordinary Items and Discontinued Operations 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000
Net Income after Non-Controlling/Minority Interests 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000
Net Income Available to Common Stockholders 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000
Diluted Net Income Available to Common Stockholders 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000
Income Statement Supplemental Section
Reported Normalized and Operating Income/Expense Supplemental Section
Total Revenue as Reported, Supplemental 2.57637E+11 75325000000 65118000000 61880000000 55314000000 56898000000 46173000000 38297000000
Total Operating Profit/Loss as Reported, Supplemental 78714000000 21885000000 21031000000 19361000000 16437000000 15651000000 11213000000 6383000000
Reported Effective Tax Rate 0.162 0.179 0.157 0.158 0.158 0.159
Reported Normalized Income
Reported Normalized Operating Profit
Other Adjustments to Net Income Available to Common Stockholders
Discontinued Operations
Basic EPS 113.88 31.15 28.44 27.69 26.63 22.54 16.55 10.21
Basic EPS from Continuing Operations 113.88 31.12 28.44 27.69 26.63 22.46 16.55 10.21
Basic EPS from Discontinued Operations
Diluted EPS 112.2 30.69 27.99 27.26 26.29 22.3 16.4 10.13
Diluted EPS from Continuing Operations 112.2 30.67 27.99 27.26 26.29 22.23 16.4 10.13
Diluted EPS from Discontinued Operations
Basic Weighted Average Shares Outstanding 667650000 662664000 665758000 668958000 673220000 675581000 679449000 681768000
Diluted Weighted Average Shares Outstanding 677674000 672493000 676519000 679612000 682071000 682969000 685851000 687024000
Reported Normalized Diluted EPS
Basic EPS 113.88 31.15 28.44 27.69 26.63 22.54 16.55 10.21
Diluted EPS 112.2 30.69 27.99 27.26 26.29 22.3 16.4 10.13
Basic WASO 667650000 662664000 665758000 668958000 673220000 675581000 679449000 681768000
Diluted WASO 677674000 672493000 676519000 679612000 682071000 682969000 685851000 687024000
Fiscal year end September 28th., 2022. | USD
Your federal taxable wages this period are $
ALPHABET INCOME Advice number:
1600 AMPIHTHEATRE  PARKWAY MOUNTAIN VIEW CA 94043 2.21169E+13




GOOGL_income-statement_Quarterly_As_Originally_Reported Q4 2021 Q3 2021 Q2 2021 Q1 2021 Q4 2020
Cash Flow from Operating Activities, Indirect 24934000000 25539000000 37497000000 31211000000 30818000000
Net Cash Flow from Continuing Operating Activities, Indirect 24934000000 25539000000 21890000000 19289000000 22677000000
Cash Generated from Operating Activities 24934000000 25539000000 21890000000 19289000000 22677000000
Income/Loss before Non-Cash Adjustment 20642000000 18936000000 18525000000 17930000000 15227000000
Total Adjustments for Non-Cash Items 6517000000 3797000000 4236000000 2592000000 5748000000
Depreciation, Amortization and Depletion, Non-Cash Adjustment 3439000000 3304000000 2945000000 2753000000 3725000000
Depreciation and Amortization, Non-Cash Adjustment 3439000000 3304000000 2945000000 2753000000 3725000000
Depreciation, Non-Cash Adjustment 3215000000 3085000000 2730000000 2525000000 3539000000
Amortization, Non-Cash Adjustment 224000000 219000000 215000000 228000000 186000000
Stock-Based Compensation, Non-Cash Adjustment 3954000000 3874000000 3803000000 3745000000 3223000000
Taxes, Non-Cash Adjustment 1616000000 -1287000000 379000000 1100000000 1670000000
Investment Income/Loss, Non-Cash Adjustment -2478000000 -2158000000 -2883000000 -4751000000 -3262000000
Gain/Loss on Financial Instruments, Non-Cash Adjustment -2478000000 -2158000000 -2883000000 -4751000000 -3262000000
Other Non-Cash Items -14000000 64000000 -8000000 -255000000 392000000
Changes in Operating Capital -2225000000 2806000000 -871000000 -1233000000 1702000000
Change in Trade and Other Receivables -5819000000 -2409000000 -3661000000 2794000000 -5445000000
Change in Trade/Accounts Receivable -5819000000 -2409000000 -3661000000 2794000000 -5445000000
Change in Other Current Assets -399000000 -1255000000 -199000000 7000000 -738000000
Change in Payables and Accrued Expenses 6994000000 3157000000 4074000000 -4956000000 6938000000
Change in Trade and Other Payables 1157000000 238000000 -130000000 -982000000 963000000
Change in Trade/Accounts Payable 1157000000 238000000 -130000000 -982000000 963000000
Change in Accrued Expenses 5837000000 2919000000 4204000000 -3974000000 5975000000
Change in Deferred Assets/Liabilities 368000000 272000000 -3000000 137000000 207000000
Change in Other Operating Capital -3369000000 3041000000 -1082000000 785000000 740000000
Change in Prepayments and Deposits
Cash Flow from Investing Activities -11016000000 -10050000000 -9074000000 -5383000000 -7281000000
Cash Flow from Continuing Investing Activities -11016000000 -10050000000 -9074000000 -5383000000 -7281000000
Purchase/Sale and Disposal of Property, Plant and Equipment, Net -6383000000 -6819000000 -5496000000 -5942000000 -5479000000
Purchase of Property, Plant and Equipment -6383000000 -6819000000 -5496000000 -5942000000 -5479000000
Sale and Disposal of Property, Plant and Equipment
Purchase/Sale of Business, Net -385000000 -259000000 -308000000 -1666000000 -370000000
Purchase/Acquisition of Business -385000000 -259000000 -308000000 -1666000000 -370000000
Purchase/Sale of Investments, Net -4348000000 -3360000000 -3293000000 2195000000 -1375000000
Purchase of Investments -40860000000 -35153000000 -24949000000 -37072000000 -36955000000
Sale of Investments 36512000000 31793000000 21656000000 39267000000 35580000000
Other Investing Cash Flow 100000000 388000000 23000000 30000000 -57000000
Purchase/Sale of Other Non-Current Assets, Net
Sales of Other Non-Current Assets
Cash Flow from Financing Activities -16511000000 -15254000000 -15991000000 -13606000000 -9270000000
Cash Flow from Continuing Financing Activities -16511000000 -15254000000 -15991000000 -13606000000 -9270000000
Issuance of/Payments for Common Stock, Net -13473000000 -12610000000 -12796000000 -11395000000 -7904000000
Payments for Common Stock 13473000000 -12610000000 -12796000000 -11395000000 -7904000000
Proceeds from Issuance of Common Stock
Issuance of/Repayments for Debt, Net 115000000 -42000000 -1042000000 -37000000 -57000000
Issuance of/Repayments for Long Term Debt, Net 115000000 -42000000 -1042000000 -37000000 -57000000
Proceeds from Issuance of Long Term Debt 6250000000 6350000000 6699000000 900000000 0
Repayments for Long Term Debt 6365000000 -6392000000 -7741000000 -937000000 -57000000
Proceeds from Issuance/Exercising of Stock Options/Warrants 2923000000 -2602000000 -2453000000 -2184000000 -1647000000


Other Financing Cash Flow 0
Cash and Cash Equivalents, End of Period 20945000000 23719000000 300000000 10000000 338000000000)
Change in Cash 25930000000 235000000000) 23630000000 26622000000 26465000000
Effect of Exchange Rate Changes 181000000000) -146000000000) -3175000000 300000000 6126000000
Cash and Cash Equivalents, Beginning of Period 2.3719E+13 2.363E+13 183000000 -143000000 210000000
Cash Flow Supplemental Section 2774000000) 89000000 266220000000000) 26465000000000) 20129000000000)
Change in Cash as Reported, Supplemental 13412000000 157000000 -2992000000 6336000000
Income Tax Paid, Supplemental 2774000000 89000000 2.2677E+15 -4990000000
Cash and Cash Equivalents, Beginning of Period

12 Months Ended
_________________________________________________________
Q4 2020 Q4  2019
Income Statement
USD in "000'"s
Repayments for Long Term Debt Dec. 31, 2020 Dec. 31, 2019
Costs and expenses:
Cost of revenues 182527 161857
Research and development
Sales and marketing 84732 71896
General and administrative 27573 26018
European Commission fines 17946 18464
Total costs and expenses 11052 9551
Income from operations 0 1697
Other income (expense), net 141303 127626
Income before income taxes 41224 34231
Provision for income taxes 6858000000 5394
Net income 22677000000 19289000000
*include interest paid, capital obligation, and underweighting 22677000000 19289000000
22677000000 19289000000
Basic net income per share of Class A and B common stock and Class C capital stock (in dollars par share)
Diluted net income per share of Class A and Class B common stock and Class C capital stock (in dollars par share)


For Disclosure, Privacy Act, and Paperwork Reduction Act Notice, see the seperate Instructions.

Returned for Signature
Date.                                                               2022-09-01

IRS RECIEVED













































































Wood.,  Zachry T.   S.R.O. Tax Period Total
Fed 941 Corporate 2007-09-30 66986.66
Fed 941 West Subsidiary 2007-09-30 17115.41
Fed 941 South Subsidiary 2007-09-30 23906.09
Fed 941 East Subsidiary 2007-09-30 11247.64
Fed 941 Corp - Penalty 2007-09-30 27198.5
Fed 940 Annual Unemp - Corp 2007-09-30 17028.05


ID: TxDL: 00037305581 Ssn: 633-44-1725

On Fri, Nov 11, 2022 at 8:55 PM ZACHRY WOOD <zachryiixixiiwood@gmail.com> wrote:
Employee Number: 999999998 IRS No.:0000000000 
Description Amount 5/4/2022 - 6/4/2022
Payment Amount (Total) 9246754678763 Display All
1. Social Security (Employee + Employer) 26662
2. Medicare (Employee + Employer) 861193422444 Hourly
3. Federal Income Tax 8385561229657 00000
Note: This report is generated based on the payroll data for your reference only. Please contact IRS office for special cases such as late payment, previous overpayment, penalty and others.
Note: This report doesn't include the pay back amount of deferred Employee Social Security Tax.
Employer Customized Report
ADP
Report Range5/4/2022 - 6/4/2022 88-1656496 state ID: 633441725 Ssn :XXXXX1725 State: All Local ID: 00037305581 2267700
EIN:
Customized Report Amount Employee Payment Report
ADP
Employee Number: 3
Description Home > Chapter 7: Reports > Custom Reports > Exporting Custom Reports > Export Custom Report as Excel File
Wages, Tips and Other Compensation 22662983361014 Tips
Taxable SS Wages 215014 5105000
Taxable SS Tips 00000
Taxable Medicare Wages 22662983361014 Salary Vacation hourly OT
Advanced EIC Payment 00000 3361014
Federal Income Tax Withheld 8385561229657 Bonus 00000 00000
Employee SS Tax Withheld 13331 00000 Other Wages 1 Other Wages 2
Employee Medicare Tax Withheld 532580113436 Total 00000 00000
State Income Tax Withheld 00000 22662983361014
Local Income Tax Withheld
Customized Employer Tax Report 00000 Deduction Summary
Description Amount Health Insurance
Employer SS Tax
Employer Medicare Tax 13331 00000
Federal Unemployment Tax 328613309009 Tax Summary
State Unemployment Tax 00442 Federal Tax 00007 Total Tax
Customized Deduction Report 00840 $8,385,561,229,657@3,330.90 Local Tax
Health Insurance 00000
401K 00000 Advanced EIC Payment 8918141356423
00000 00000 Total
401K
00000 00000

ZACHRY T WOOD Social Security Tax Medicare Tax State Tax 532580113050


SHAREHOLDERS ARE URGED TO READ THE DEFINITIVE PROXY STATEMENT AND ANY OTHER RELEVANT MATERIALS THAT THE COMPANY WILL FILE WITH THE SEC CAREFULLY IN THEIR ENTIRETY WHEN THEY BECOME AVAILABLE. SUCH DOCUMENTS WILL CONTAIN IMPORTANT INFORMATION ABOUT THE COMPANY AND ITS DIRECTORS, OFFICERS AND AFFILIATES. INFORMATION REGARDING THE INTERESTS OF CERTAIN OF THE COMPANY’S DIRECTORS, OFFICERS AND AFFILIATES WILL BE AVAILABLE IN THE DEFINITIVE PROXY STATEMENT.
The Definitive Proxy Statement and any other relevant materials that will be filed with the SEC will be available free of charge at the SEC’s website at www.sec.gov. In addition, the Definitive Proxy Statement (when available) and other relevant documents will also be available, without charge, by directing a request by mail to Attn: Investor Relations, Alphabet Inc., 1600 Amphitheatre Parkway, Mountain View, California, 94043 or by contacting investor-relations@abc.xyz. The Definitive Proxy Statement and other relevant documents will also be available on the Company’s Investor Relations website at https://abc.xyz/investor/other/annual-meeting/.

The Company and its directors and certain of its executive officers may be consideredno participants in the solicitation of proxies with respect to the proposals under the Definitive Proxy Statement under the rules of the SEC. Additional information regarding the participants in the proxy solicitations and a description of their direct and indirect interests, by security holdings or otherwise, also will be included in the Definitive Proxy Statement and other relevant materials to be filed with the SEC when they become available. . 9246754678763




3/6/2022 at 6:37 PM
Q4 2021 Q3 2021 Q2 2021 Q1 2021 Q4 2020

GOOGL_income-statement_Quarterly_As_Originally_Reported 24934000000 25539000000 37497000000 31211000000 30818000000
24934000000 25539000000 21890000000 19289000000 22677000000
Cash Flow from Operating Activities, Indirect 24934000000 25539000000 21890000000 19289000000 22677000000
Net Cash Flow from Continuing Operating Activities, Indirect 20642000000 18936000000 18525000000 17930000000 15227000000
Cash Generated from Operating Activities 6517000000 3797000000 4236000000 2592000000 5748000000
Income/Loss before Non-Cash Adjustment 3439000000 3304000000 2945000000 2753000000 3725000000
Total Adjustments for Non-Cash Items 3439000000 3304000000 2945000000 2753000000 3725000000
Depreciation, Amortization and Depletion, Non-Cash Adjustment 3215000000 3085000000 2730000000 2525000000 3539000000
Depreciation and Amortization, Non-Cash Adjustment 224000000 219000000 215000000 228000000 186000000
Depreciation, Non-Cash Adjustment 3954000000 3874000000 3803000000 3745000000 3223000000
Amortization, Non-Cash Adjustment 1616000000 -1287000000 379000000 1100000000 1670000000
Stock-Based Compensation, Non-Cash Adjustment -2478000000 -2158000000 -2883000000 -4751000000 -3262000000
Taxes, Non-Cash Adjustment -2478000000 -2158000000 -2883000000 -4751000000 -3262000000
Investment Income/Loss, Non-Cash Adjustment -14000000 64000000 -8000000 -255000000 392000000
Gain/Loss on Financial Instruments, Non-Cash Adjustment -2225000000 2806000000 -871000000 -1233000000 1702000000
Other Non-Cash Items -5819000000 -2409000000 -3661000000 2794000000 -5445000000
Changes in Operating Capital -5819000000 -2409000000 -3661000000 2794000000 -5445000000
Change in Trade and Other Receivables -399000000 -1255000000 -199000000 7000000 -738000000
Change in Trade/Accounts Receivable 6994000000 3157000000 4074000000 -4956000000 6938000000
Change in Other Current Assets 1157000000 238000000 -130000000 -982000000 963000000
Change in Payables and Accrued Expenses 1157000000 238000000 -130000000 -982000000 963000000
Change in Trade and Other Payables 5837000000 2919000000 4204000000 -3974000000 5975000000
Change in Trade/Accounts Payable 368000000 272000000 -3000000 137000000 207000000
Change in Accrued Expenses -3369000000 3041000000 -1082000000 785000000 740000000
Change in Deferred Assets/Liabilities
Change in Other Operating Capital
-11016000000 -10050000000 -9074000000 -5383000000 -7281000000
Change in Prepayments and Deposits -11016000000 -10050000000 -9074000000 -5383000000 -7281000000
Cash Flow from Investing Activities
Cash Flow from Continuing Investing Activities -6383000000 -6819000000 -5496000000 -5942000000 -5479000000
-6383000000 -6819000000 -5496000000 -5942000000 -5479000000
Purchase/Sale and Disposal of Property, Plant and Equipment, Net
Purchase of Property, Plant and Equipment -385000000 -259000000 -308000000 -1666000000 -370000000
Sale and Disposal of Property, Plant and Equipment -385000000 -259000000 -308000000 -1666000000 -370000000
Purchase/Sale of Business, Net -4348000000 -3360000000 -3293000000 2195000000 -1375000000
Purchase/Acquisition of Business -40860000000 -35153000000 -24949000000 -37072000000 -36955000000
Purchase/Sale of Investments, Net
Purchase of Investments 36512000000 31793000000 21656000000 39267000000 35580000000
100000000 388000000 23000000 30000000 -57000000
Sale of Investments
Other Investing Cash Flow -15254000000
Purchase/Sale of Other Non-Current Assets, Net -16511000000 -15254000000 -15991000000 -13606000000 -9270000000
Sales of Other Non-Current Assets -16511000000 -12610000000 -15991000000 -13606000000 -9270000000
Cash Flow from Financing Activities -13473000000 -12610000000 -12796000000 -11395000000 -7904000000
Cash Flow from Continuing Financing Activities 13473000000 -12796000000 -11395000000 -7904000000
Issuance of/Payments for Common 343 sec cvxvxvcclpddf wearsStock, Net -42000000
Payments for Common Stock 115000000 -42000000 -1042000000 -37000000 -57000000
Proceeds from Issuance of Common Stock 115000000 6350000000 -1042000000 -37000000 -57000000
Issuance of/Repayments for Debt, Net 6250000000 -6392000000 6699000000 900000000 00000
Issuance of/Repayments for Long Term Debt, Net 6365000000 -2602000000 -7741000000 -937000000 -57000000
Proceeds from Issuance of Long Term Debt
Repayments for Long Term Debt 2923000000 -2453000000 -2184000000 -1647000000

Proceeds from Issuance/Exercising of Stock Options/Warrants 00000 300000000 10000000 338000000000
Other Financing Cash Flow
Cash and Cash Equivalents, End of Period
Change in Cash 20945000000 23719000000 23630000000 26622000000 26465000000
Effect of Exchange Rate Changes 25930000000) 235000000000 -3175000000 300000000 6126000000
Cash and Cash Equivalents, Beginning of Period PAGE="$USD(181000000000)".XLS BRIN="$USD(146000000000)".XLS 183000000 -143000000 210000000
Cash Flow Supplemental Section 23719000000000 26622000000000 26465000000000 20129000000000
Change in Cash as Reported, Supplemental 2774000000 89000000 -2992000000 6336000000
Income Tax Paid, Supplemental 13412000000 157000000
ZACHRY T WOOD -4990000000
Cash and Cash Equivalents, Beginning of Period
Department of the Treasury
Internal Revenue Service
Q4 2020 Q4  2019
Calendar Year
Due: 04/18/2022
Dec. 31, 2020 Dec. 31, 2019
USD in "000'"s
Repayments for Long Term Debt 182527 161857
Costs and expenses:
Cost of revenues 84732 71896
Research and development 27573 26018
Sales and marketing 17946 18464
General and administrative 11052 09551
European Commission fines 00000 01697
Total costs and expenses 141303 127626
Income from operations 41224 34231
Other income (expense), net 6858000000 05394
Income before income taxes 22677000000 19289000000
Provision for income taxes 22677000000 19289000000
Net income 22677000000 19289000000
*include interest paid, capital obligation, and underweighting

Basic net income per share of Class A and B common stock and Class C capital stock (in dollars par share)










Diluted net income per share of Class A and Class B common stock and Class C capital stock (in dollars par share)
*include interest paid, capital obligation, and underweighting

Basic net income per share of Class A and B common stock and Class C capital stock (in dollars par share)
Diluted net income per share of Class A and Class B common stock and Class C capital stock (in dollars par share)







20210418
Rate Units Total YTD Taxes / Deductions Current YTD
- - 70842745000 70842745000 Federal Withholding 00000 188813800
FICA - Social Security 00000 853700
FICA - Medicare 00000 11816700
Employer Taxes
FUTA 00000 00000
SUTA 00000 00000
EIN: 61-1767919 ID : 00037305581 SSN: 633441725 ATAA Payments 00000 102600

Gross
70842745000 Earnings Statement
Taxes / Deductions Stub Number: 1
00000
Net Pay SSN Pay Schedule Pay Period Sep 28, 2022 to Sep 29, 2023 Pay Date 4/18/2022
70842745000 XXX-XX-1725 Annually
CHECK NO.
5560149





INTERNAL REVENUE SERVICE,
PO BOX 1214,
CHARLOTTE, NC 28201-1214

ZACHRY WOOD
00015 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000 6836000000 10671000000 7068000000
For Disclosure, Privacy Act, and Paperwork Reduction Act Notice, see separate instructions. 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000 6836000000 10671000000 7068000000
Cat. No. 11320B 76033000000 20642000000 18936000000 18525000000 17930000000 15227000000 11247000000 6959000000 6836000000 10671000000 7068000000
Form 1040 (2021) 76033000000 20642000000 18936000000
Reported Normalized and Operating Income/Expense Supplemental Section
Total Revenue as Reported, Supplemental 257637000000 75325000000 65118000000 61880000000 55314000000 56898000000 46173000000 38297000000 41159000000 46075000000 40499000000
Total Operating Profit/Loss as Reported, Supplemental 78714000000 21885000000 21031000000 19361000000 16437000000 15651000000 11213000000 6383000000 7977000000 9266000000 9177000000
Reported Effective Tax Rate 00000 00000 00000 00000 00000 00000 00000 00000 00000
Reported Normalized Income 6836000000
Reported Normalized Operating Profit 7977000000
Other Adjustments to Net Income Available to Common Stockholders
Discontinued Operations
Basic EPS 00114 00031 00028 00028 00027 00023 00017 00010 00010 00015 00010
Basic EPS from Continuing Operations 00114 00031 00028 00028 00027 00022 00017 00010 00010 00015 00010
Basic EPS from Discontinued Operations
Diluted EPS 00112 00031 00028 00027 00026 00022 00016 00010 00010 00015 00010
Diluted EPS from Continuing Operations 00112 00031 00028 00027 00026 00022 00016 00010 00010 00015 00010
Diluted EPS from Discontinued Operations
Basic Weighted Average Shares Outstanding 667650000 662664000 665758000 668958000 673220000 675581000 679449000 681768000 686465000 688804000 692741000
Diluted Weighted Average Shares Outstanding 677674000 672493000 676519000 679612000 682071000 682969000 685851000 687024000 692267000 695193000 698199000
Reported Normalized Diluted EPS 00010
Basic EPS 00114 00031 00028 00028 00027 00023 00017 00010 00010 00015 00010 00001
Diluted EPS 00112 00031 00028 00027 00026 00022 00016 00010 00010 00015 00010
Basic WASO 667650000 662664000 665758000 668958000 673220000 675581000 679449000 681768000 686465000 688804000 692741000
Diluted WASO 677674000 672493000 676519000 679612000 682071000 682969000 685851000 687024000 692267000 695193000 698199000
Fiscal year end September 28th., 2022. | USD

On Fri, Nov 11, 2022 at 8:51 PM Carolyn Robbins <ckrobbins70@gmail.com> wrote:

🥰🥰
On Fri, Nov 11, 2022 at 3:00 PM ZACHRY WOOD <zachryiixixiiwood@gmail.com> wrote:
I LOVE YOUR MORE GOOBERSTEIN!!

On Fri, Nov 11, 2022 at 6:53 AM Carolyn Robbins <ckrobbins70@gmail.com> wrote:
I love you 💕 

On Fri, Nov 11, 2022 at 1:14 AM ZACHRY WOOD <zachryiixixiiwood@gmail.com> wrote:
2021/09/29					2880										
Paid Period	09-28-2019 - 09 28-2021										
Pay Date	01-29-2022										
89															
6551				Amount										
$70,432,743,866										
total											
Alphabet Inc.					$134,839										
Income Statement															
Zachry Tyler Wood															
US$ in millions	Dec 31, 2019	Dec 31, 2018	Dec 31, 2017	Dec 31, 2016	Dec 31, 2015										
Ann. Rev. Date	161,857	136,819
:Build::
Publish:
Launch:
Rlease:
repositories'@0071921891$4720416547 ::
Deployee ::
Return: 'Run ''
