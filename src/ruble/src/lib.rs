use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result};
use base64::Engine;
use printpdf::{
    BuiltinFont, Color, IndirectFontRef, Line, Mm, PdfDocument, PdfLayerReference, Point, Rgb,
};
use roxmltree::{Document, Node};

#[derive(Debug, Clone)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub postal: String,
}

#[derive(Debug, Clone)]
pub struct InvoiceLine {
    pub description: String,
    pub quantity: String,
    pub unit_price: String,
    pub total: String,
}

#[derive(Debug, Clone)]
pub struct InvoiceData {
    pub invoice_number: String,
    pub issue_date: String,
    pub due_date: String,
    pub currency: String,
    pub supplier_name: String,
    pub supplier_vat: String,
    pub supplier_address: Address,
    pub customer_name: String,
    pub customer_vat: String,
    pub customer_address: Address,
    pub subtotal: String,
    pub tax_total: String,
    pub total: String,
    pub lines: Vec<InvoiceLine>,
}

#[derive(Debug, Clone)]
pub struct EmbeddedPdf {
    pub filename: Option<String>,
    pub bytes: Vec<u8>,
}

pub fn parse_ubl_invoice(xml: &str) -> Result<InvoiceData> {
    let doc = Document::parse(xml).context("parse XML")?;
    let root = doc.root_element();

    let invoice_number = find_text(&root, "ID").unwrap_or_default();
    let issue_date = find_text(&root, "IssueDate").unwrap_or_default();
    let due_date = find_text(&root, "DueDate").unwrap_or_default();
    let currency = find_text(&root, "DocumentCurrencyCode").unwrap_or_default();

    let supplier_party =
        find_descendant(root, "AccountingSupplierParty").and_then(|node| find_descendant(node, "Party"));
    let supplier_name = supplier_party
        .as_ref()
        .and_then(|node| find_text(node, "Name"))
        .unwrap_or_default();
    let supplier_vat = supplier_party
        .as_ref()
        .and_then(|node| find_text(node, "CompanyID"))
        .unwrap_or_default();
    let supplier_address = parse_address(supplier_party.as_ref());

    let customer_party =
        find_descendant(root, "AccountingCustomerParty").and_then(|node| find_descendant(node, "Party"));
    let customer_name = customer_party
        .as_ref()
        .and_then(|node| find_text(node, "Name"))
        .unwrap_or_default();
    let customer_vat = customer_party
        .as_ref()
        .and_then(|node| find_text(node, "CompanyID"))
        .unwrap_or_default();
    let customer_address = parse_address(customer_party.as_ref());

    let legal_total = find_descendant(root, "LegalMonetaryTotal");
    let subtotal = legal_total
        .as_ref()
        .and_then(|node| find_text(node, "TaxExclusiveAmount"))
        .unwrap_or_default();
    let total = legal_total
        .as_ref()
        .and_then(|node| find_text(node, "TaxInclusiveAmount"))
        .unwrap_or_default();
    let tax_total = find_text(&root, "TaxAmount").unwrap_or_default();

    let mut lines = Vec::new();
    for line_node in root
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "InvoiceLine")
    {
        let line = InvoiceLine {
            description: find_text(&line_node, "Description").unwrap_or_default(),
            quantity: find_text(&line_node, "InvoicedQuantity").unwrap_or_default(),
            unit_price: find_text(&line_node, "PriceAmount").unwrap_or_default(),
            total: find_text(&line_node, "LineExtensionAmount").unwrap_or_default(),
        };
        lines.push(line);
    }

    Ok(InvoiceData {
        invoice_number,
        issue_date,
        due_date,
        currency,
        supplier_name,
        supplier_vat,
        supplier_address,
        customer_name,
        customer_vat,
        customer_address,
        subtotal,
        tax_total,
        total,
        lines,
    })
}

pub fn extract_embedded_pdf(xml: &str) -> Result<Option<EmbeddedPdf>> {
    let doc = Document::parse(xml).context("parse XML for embedded PDF")?;
    let node = doc.descendants().find(|node| {
        node.is_element()
            && node.tag_name().name() == "EmbeddedDocumentBinaryObject"
            && node.attribute("mimeCode") == Some("application/pdf")
    });

    let Some(node) = node else {
        return Ok(None);
    };

    let payload = node.text().unwrap_or("").trim();
    if payload.is_empty() {
        return Ok(None);
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .context("decode embedded PDF")?;
    Ok(Some(EmbeddedPdf {
        filename: node.attribute("filename").map(|value| value.to_string()),
        bytes,
    }))
}

pub fn create_invoice_pdf(data: &InvoiceData, output_file: &Path) -> Result<()> {
    let (doc, page1, layer1) =
        PdfDocument::new("Invoice", Mm(210.0), Mm(297.0), "Layer 1");
    let font = doc
        .add_builtin_font(BuiltinFont::Helvetica)
        .context("load built-in font")?;
    let font_bold = doc
        .add_builtin_font(BuiltinFont::HelveticaBold)
        .context("load bold font")?;
    let layer = doc.get_page(page1).get_layer(layer1);

    let mut y = 284.0;
    let line_height = 6.5;
    let left_x = 18.0;
    let right_x = 110.0;

    layer.set_fill_color(Color::Rgb(Rgb::new(0.14, 0.22, 0.33, None)));
    write_text(&layer, &font_bold, 22.0, left_x, y, "INVOICE");
    layer.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
    y -= 10.0;

    write_text(
        &layer,
        &font_bold,
        10.0,
        left_x,
        y,
        "Invoice details",
    );
    y -= 6.0;
    write_text(
        &layer,
        &font,
        10.0,
        left_x,
        y,
        &format!("Invoice Number: {}", data.invoice_number),
    );
    write_text(
        &layer,
        &font,
        10.0,
        right_x,
        y,
        &format!("Issue Date: {}", data.issue_date),
    );
    y -= line_height;
    if !data.due_date.is_empty() {
        write_text(
            &layer,
            &font,
            10.0,
            right_x,
            y,
            &format!("Due Date: {}", data.due_date),
        );
    }

    y -= 8.0;
    draw_divider(&layer, left_x, y, 192.0);
    y -= 7.0;

    write_text(&layer, &font_bold, 11.0, left_x, y, "Supplier");
    write_text(&layer, &font_bold, 11.0, right_x, y, "Customer");
    y -= line_height;
    write_text(
        &layer,
        &font,
        10.0,
        left_x,
        y,
        &data.supplier_name,
    );
    write_text(
        &layer,
        &font,
        10.0,
        right_x,
        y,
        &data.customer_name,
    );
    y -= line_height;
    if !data.supplier_address.street.is_empty() || !data.customer_address.street.is_empty() {
        write_text(
            &layer,
            &font,
            9.5,
            left_x,
            y,
            &data.supplier_address.street,
        );
        write_text(
            &layer,
            &font,
            9.5,
            right_x,
            y,
            &data.customer_address.street,
        );
        y -= line_height;
    }
    if !data.supplier_address.city.is_empty()
        || !data.supplier_address.postal.is_empty()
        || !data.customer_address.city.is_empty()
        || !data.customer_address.postal.is_empty()
    {
        write_text(
            &layer,
            &font,
            9.5,
            left_x,
            y,
            &format!(
                "{} {}",
                data.supplier_address.postal, data.supplier_address.city
            ),
        );
        write_text(
            &layer,
            &font,
            9.5,
            right_x,
            y,
            &format!(
                "{} {}",
                data.customer_address.postal, data.customer_address.city
            ),
        );
        y -= line_height;
    }
    if !data.supplier_vat.is_empty() || !data.customer_vat.is_empty() {
        write_text(
            &layer,
            &font,
            9.5,
            left_x,
            y,
            &format!("VAT: {}", data.supplier_vat),
        );
        write_text(
            &layer,
            &font,
            9.5,
            right_x,
            y,
            &format!("VAT: {}", data.customer_vat),
        );
        y -= line_height;
    }

    y -= 6.0;
    draw_divider(&layer, left_x, y, 192.0);
    y -= 7.0;

    write_text(&layer, &font_bold, 11.0, left_x, y, "Items");
    y -= 6.0;
    layer.set_fill_color(Color::Rgb(Rgb::new(0.35, 0.35, 0.35, None)));
    write_text(&layer, &font_bold, 9.5, left_x, y, "Description");
    write_text(&layer, &font_bold, 9.5, 122.0, y, "Qty");
    write_text(&layer, &font_bold, 9.5, 145.0, y, "Unit");
    write_text(&layer, &font_bold, 9.5, 172.0, y, "Total");
    layer.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
    y -= 4.0;
    draw_divider(&layer, left_x, y, 192.0);
    y -= 6.0;

    for line in &data.lines {
        let description = line.description.clone();
        write_text(&layer, &font, 9.0, left_x, y, &description);
        write_text(&layer, &font, 9.0, 122.0, y, &line.quantity);
        write_text(
            &layer,
            &font,
            9.0,
            145.0,
            y,
            &format!("{} {}", data.currency, line.unit_price),
        );
        write_text(
            &layer,
            &font,
            9.0,
            172.0,
            y,
            &format!("{} {}", data.currency, line.total),
        );
        y -= line_height;
    }

    y -= 4.0;
    draw_divider(&layer, left_x, y, 192.0);
    y -= 7.0;
    write_text(
        &layer,
        &font,
        10.0,
        130.0,
        y,
        &format!("Subtotal: {} {}", data.currency, data.subtotal),
    );
    y -= line_height;
    write_text(
        &layer,
        &font,
        10.0,
        130.0,
        y,
        &format!("VAT: {} {}", data.currency, data.tax_total),
    );
    y -= line_height;
    write_text(
        &layer,
        &font_bold,
        12.0,
        130.0,
        y,
        &format!("Total: {} {}", data.currency, data.total),
    );

    let mut writer = BufWriter::new(File::create(output_file)?);
    doc.save(&mut writer).context("write PDF")?;
    Ok(())
}

fn parse_address(party: Option<&Node<'_, '_>>) -> Address {
    let Some(party) = party else {
        return Address {
            street: String::new(),
            city: String::new(),
            postal: String::new(),
        };
    };

    let address_node = find_descendant(*party, "PostalAddress");
    Address {
        street: address_node
            .as_ref()
            .and_then(|node| find_text(node, "StreetName"))
            .unwrap_or_default(),
        city: address_node
            .as_ref()
            .and_then(|node| find_text(node, "CityName"))
            .unwrap_or_default(),
        postal: address_node
            .as_ref()
            .and_then(|node| find_text(node, "PostalZone"))
            .unwrap_or_default(),
    }
}

fn find_descendant<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.descendants()
        .find(|child| child.is_element() && child.tag_name().name() == name)
}

fn find_text(node: &Node<'_, '_>, name: &str) -> Option<String> {
    find_descendant(*node, name)
        .and_then(|child| child.text())
        .map(|text| text.trim().to_string())
}

fn write_text(
    layer: &PdfLayerReference,
    font: &IndirectFontRef,
    size: f64,
    x: f64,
    y: f64,
    text: &str,
) {
    layer.use_text(text, size as f32, Mm(x as f32), Mm(y as f32), font);
}

fn draw_divider(layer: &PdfLayerReference, x1: f64, y: f64, x2: f64) {
    layer.set_outline_thickness(0.3);
    layer.set_outline_color(Color::Rgb(Rgb::new(0.75, 0.75, 0.75, None)));
    let line = Line {
        points: vec![
            (Point::new(Mm(x1 as f32), Mm(y as f32)), false),
            (Point::new(Mm(x2 as f32), Mm(y as f32)), false),
        ],
        is_closed: false,
    };
    layer.add_line(line);
    layer.set_outline_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
    layer.set_outline_thickness(1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_XML: &str = r#"
<Invoice xmlns="urn:oasis:names:specification:ubl:schema:xsd:Invoice-2"
 xmlns:cac="urn:oasis:names:specification:ubl:schema:xsd:CommonAggregateComponents-2"
 xmlns:cbc="urn:oasis:names:specification:ubl:schema:xsd:CommonBasicComponents-2">
  <cbc:ID>INV-1</cbc:ID>
  <cbc:IssueDate>2024-01-01</cbc:IssueDate>
  <cbc:DocumentCurrencyCode>EUR</cbc:DocumentCurrencyCode>
  <cac:AccountingSupplierParty>
    <cac:Party>
      <cbc:Name>Supplier Inc</cbc:Name>
      <cac:PartyTaxScheme>
        <cbc:CompanyID>VAT123</cbc:CompanyID>
      </cac:PartyTaxScheme>
      <cac:PostalAddress>
        <cbc:StreetName>Main</cbc:StreetName>
        <cbc:CityName>Paris</cbc:CityName>
        <cbc:PostalZone>75001</cbc:PostalZone>
      </cac:PostalAddress>
    </cac:Party>
  </cac:AccountingSupplierParty>
  <cac:AccountingCustomerParty>
    <cac:Party>
      <cbc:Name>Customer LLC</cbc:Name>
      <cac:PartyTaxScheme>
        <cbc:CompanyID>VAT999</cbc:CompanyID>
      </cac:PartyTaxScheme>
      <cac:PostalAddress>
        <cbc:StreetName>Rue 1</cbc:StreetName>
        <cbc:CityName>Lyon</cbc:CityName>
        <cbc:PostalZone>69000</cbc:PostalZone>
      </cac:PostalAddress>
    </cac:Party>
  </cac:AccountingCustomerParty>
  <cac:LegalMonetaryTotal>
    <cbc:TaxExclusiveAmount>10.00</cbc:TaxExclusiveAmount>
    <cbc:TaxInclusiveAmount>12.00</cbc:TaxInclusiveAmount>
  </cac:LegalMonetaryTotal>
  <cac:TaxTotal>
    <cbc:TaxAmount>2.00</cbc:TaxAmount>
  </cac:TaxTotal>
  <cac:InvoiceLine>
    <cbc:InvoicedQuantity>1</cbc:InvoicedQuantity>
    <cbc:LineExtensionAmount>10.00</cbc:LineExtensionAmount>
    <cac:Item>
      <cbc:Description>Widget</cbc:Description>
    </cac:Item>
    <cac:Price>
      <cbc:PriceAmount>10.00</cbc:PriceAmount>
    </cac:Price>
  </cac:InvoiceLine>
  <cac:AdditionalDocumentReference>
    <cac:Attachment>
      <cbc:EmbeddedDocumentBinaryObject mimeCode="application/pdf" filename="orig.pdf">aGVsbG8=</cbc:EmbeddedDocumentBinaryObject>
    </cac:Attachment>
  </cac:AdditionalDocumentReference>
</Invoice>
"#;

    #[test]
    fn parses_invoice_fields() {
        let data = parse_ubl_invoice(SAMPLE_XML).expect("parse invoice");
        assert_eq!(data.invoice_number, "INV-1");
        assert_eq!(data.issue_date, "2024-01-01");
        assert_eq!(data.currency, "EUR");
        assert_eq!(data.supplier_name, "Supplier Inc");
        assert_eq!(data.customer_name, "Customer LLC");
        assert_eq!(data.lines.len(), 1);
        assert_eq!(data.lines[0].description, "Widget");
    }

    #[test]
    fn extracts_embedded_pdf() {
        let embedded = extract_embedded_pdf(SAMPLE_XML)
            .expect("extract embedded")
            .expect("embedded pdf present");
        assert_eq!(embedded.filename.as_deref(), Some("orig.pdf"));
        assert_eq!(embedded.bytes, b"hello");
    }
}
