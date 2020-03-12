package de.sve.backend.mail;

import java.util.Properties;

import javax.activation.DataHandler;
import javax.mail.Address;
import javax.mail.Authenticator;
import javax.mail.BodyPart;
import javax.mail.Message;
import javax.mail.MessagingException;
import javax.mail.Multipart;
import javax.mail.PasswordAuthentication;
import javax.mail.Session;
import javax.mail.Transport;
import javax.mail.internet.InternetAddress;
import javax.mail.internet.MimeBodyPart;
import javax.mail.internet.MimeMessage;
import javax.mail.internet.MimeMultipart;

/**
 * Resposible for sending emails.
 * 
 * @author mseele
 */
public class Postman {

	protected static void deliver(Mail mail) throws MessagingException {
		MailAccount account = mail.sender();
		// smtp properties
		Properties properties = System.getProperties();
		properties.put("mail.smtp.host", "smtp.gmail.com"); //$NON-NLS-1$ //$NON-NLS-2$
		properties.put("mail.smtp.port", "465"); //$NON-NLS-1$ //$NON-NLS-2$
		properties.put("mail.smtp.ssl.enable", "true"); //$NON-NLS-1$ //$NON-NLS-2$
		properties.put("mail.smtp.auth", "true"); //$NON-NLS-1$ //$NON-NLS-2$
		// authenticate
		Session session = Session.getInstance(properties, new Authenticator() {
			@Override
			protected PasswordAuthentication getPasswordAuthentication() {
				return new PasswordAuthentication(account.userName(), account.password());
			}
		});
		MimeMessage message = new MimeMessage(session);
		// email addresses
		message.setFrom(new InternetAddress(account.email()));
		for (String recipient : mail.to()) {
			message.addRecipient(Message.RecipientType.TO, new InternetAddress(recipient));
		}
		for (String recipient : mail.bcc()) {
			message.addRecipient(Message.RecipientType.BCC, new InternetAddress(recipient));
		}
		String replyTo = mail.replyTo();
		if (replyTo != null) {
			message.setReplyTo(new Address[] { new InternetAddress(replyTo) });
		}
        Multipart multipart = new MimeMultipart();
		// subject
		message.setSubject(mail.subject());
		// content
        BodyPart content = new MimeBodyPart();
        content.setText(mail.content());
        multipart.addBodyPart(content);
        // attachments
        for (Attachment attachment : mail.attachments()) {
        	BodyPart bodyPart = new MimeBodyPart();
            bodyPart.setDataHandler(new DataHandler(attachment.dataSource()));
            bodyPart.setFileName(attachment.name());
            multipart.addBodyPart(bodyPart);
		}
		message.setContent(multipart);
		// Send message
		Transport.send(message);
	}

}
